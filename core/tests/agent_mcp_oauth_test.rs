//! Acceptance for OAuth on the agent MCP endpoint
//! (`core::service::agent_mcp::oauth`).
//!
//! Walks the flow an MCP client runs when it holds only the endpoint URL:
//! the 401 challenge, protected resource and authorization server metadata,
//! dynamic client registration, the consent page and its PKCE code, the token
//! exchange, a tool call with the issued token, refresh rotation, and refusal
//! once the owner revokes the grant. It also checks the refusals the flow
//! owes: a cross-origin consent post, a wrong verifier, a reused code, a
//! replayed refresh token, and RFC 7009 revocation by the client.

#![cfg(unix)]

mod helpers;

use std::{collections::HashMap, path::Path};

use base64::Engine;
use helpers::*;
use reqwest::{header, redirect::Policy, StatusCode};
use sd_core::{
	infra::{action::LibraryAction, db::entities::agent_grant},
	ops::grants::{
		revoke::{RevokeGrantAction, RevokeGrantInput},
		GrantCap,
	},
	service::agent_mcp::{self, Limits},
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const REDIRECT: &str = "http://127.0.0.1:39999/callback";

trait JsonBody {
	fn json_body(self, value: &Value) -> Self;
}

impl JsonBody for reqwest::RequestBuilder {
	fn json_body(self, value: &Value) -> Self {
		self.header(header::CONTENT_TYPE, "application/json")
			.body(value.to_string())
	}
}

async fn body(response: reqwest::Response) -> Value {
	let bytes = response.bytes().await.expect("body");
	serde_json::from_slice(&bytes).unwrap_or(Value::Null)
}

struct Flow {
	http: reqwest::Client,
	base: String,
	client_id: String,
}

struct Tokens {
	access: String,
	refresh: String,
}

fn pkce() -> (String, String) {
	let verifier = format!(
		"{}-{}",
		uuid::Uuid::new_v4().simple(),
		uuid::Uuid::new_v4().simple()
	);
	let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD
		.encode(Sha256::digest(verifier.as_bytes()));
	(verifier, challenge)
}

fn query_of(location: &str) -> HashMap<String, String> {
	url::Url::parse(location)
		.expect("redirect url")
		.query_pairs()
		.map(|(k, v)| (k.to_string(), v.to_string()))
		.collect()
}

impl Flow {
	/// Open the consent page and return the request id it embeds.
	async fn consent_page(&self, challenge: &str, state: &str) -> String {
		let response = self
			.http
			.get(format!("{}/oauth/authorize", self.base))
			.query(&[
				("response_type", "code"),
				("client_id", self.client_id.as_str()),
				("redirect_uri", REDIRECT),
				("code_challenge", challenge),
				("code_challenge_method", "S256"),
				("state", state),
				("scope", "read"),
				("resource", &format!("{}/mcp", self.base)),
			])
			.send()
			.await
			.expect("authorize");
		assert_eq!(response.status(), StatusCode::OK);
		assert_eq!(
			response.headers()[header::X_FRAME_OPTIONS],
			"DENY",
			"the consent page cannot be framed"
		);
		let page = response.text().await.unwrap();
		assert!(
			page.contains("OAuth acceptance"),
			"the page names the client"
		);
		let marker = "name=\"request\" value=\"";
		let start = page.find(marker).expect("request id") + marker.len();
		page[start..start + 64].to_string()
	}

	/// The owner approves `path` on the consent page; returns the code.
	async fn approve(&self, path: &Path, challenge: &str) -> String {
		let request = self.consent_page(challenge, "s1").await;
		let response = self
			.http
			.post(format!("{}/oauth/authorize", self.base))
			.header(header::ORIGIN, &self.base)
			.form(&[
				("request", request.as_str()),
				("path", path.to_str().unwrap()),
				("access", "read"),
				("ttl", "3600"),
				("decision", "approve"),
			])
			.send()
			.await
			.expect("consent");
		assert_eq!(response.status(), StatusCode::SEE_OTHER);
		let location = response.headers()[header::LOCATION].to_str().unwrap();
		assert!(location.starts_with(REDIRECT), "{location}");
		let query = query_of(location);
		assert_eq!(query["state"], "s1");
		assert_eq!(query["iss"], self.base);
		query["code"].clone()
	}

	async fn token(&self, form: &[(&str, &str)]) -> (StatusCode, Value) {
		let response = self
			.http
			.post(format!("{}/oauth/token", self.base))
			.form(form)
			.send()
			.await
			.expect("token");
		let status = response.status();
		(status, body(response).await)
	}

	async fn exchange(&self, code: &str, verifier: &str) -> (StatusCode, Value) {
		self.token(&[
			("grant_type", "authorization_code"),
			("code", code),
			("redirect_uri", REDIRECT),
			("client_id", &self.client_id),
			("code_verifier", verifier),
			("resource", &format!("{}/mcp", self.base)),
		])
		.await
	}

	async fn refresh(&self, refresh: &str) -> (StatusCode, Value) {
		self.token(&[
			("grant_type", "refresh_token"),
			("refresh_token", refresh),
			("client_id", &self.client_id),
		])
		.await
	}

	/// A full authorization: consent, then the code for tokens.
	async fn connect(&self, path: &Path) -> Tokens {
		let (verifier, challenge) = pkce();
		let code = self.approve(path, &challenge).await;
		let (status, body) = self.exchange(&code, &verifier).await;
		assert_eq!(status, StatusCode::OK, "{body}");
		assert_eq!(body["token_type"], "Bearer");
		assert_eq!(body["scope"], "read");
		let expires_in = body["expires_in"].as_i64().unwrap();
		assert!((3500..=3600).contains(&expires_in), "{expires_in}");
		Tokens {
			access: body["access_token"].as_str().unwrap().to_string(),
			refresh: body["refresh_token"].as_str().unwrap().to_string(),
		}
	}

	/// Initialize a session with `token` and read the fixture file.
	async fn read_readme(&self, token: &str) -> (StatusCode, Value) {
		let mcp = format!("{}/mcp", self.base);
		let initialize = self
			.http
			.post(&mcp)
			.bearer_auth(token)
			.header(header::ACCEPT, "application/json, text/event-stream")
			.json_body(&json!({
				"jsonrpc": "2.0", "id": 1, "method": "initialize",
				"params": {
					"protocolVersion": "2025-06-18",
					"capabilities": {},
					"clientInfo": { "name": "oauth acceptance", "version": "0" },
				},
			}))
			.send()
			.await
			.expect("initialize");
		if initialize.status() != StatusCode::OK {
			let status = initialize.status();
			return (status, body(initialize).await);
		}
		let session = initialize.headers()["mcp-session-id"]
			.to_str()
			.unwrap()
			.to_string();
		let response = self
			.http
			.post(&mcp)
			.bearer_auth(token)
			.header("mcp-session-id", session)
			.header(header::ACCEPT, "application/json, text/event-stream")
			.json_body(&json!({
				"jsonrpc": "2.0", "id": 2, "method": "tools/call",
				"params": { "name": "read", "arguments": { "path": "readme.md" } },
			}))
			.send()
			.await
			.expect("tools/call");
		let status = response.status();
		(status, body(response).await)
	}
}

#[tokio::test]
async fn an_mcp_client_connects_with_oauth_and_loses_access_on_revoke() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("agent_mcp_oauth")
		.disable_watcher()
		.build()
		.await?;
	let tracked = harness.create_test_dir("tracked").await?;
	tracked
		.write_file("project/readme.md", "# Readme\nhello from oauth\n")
		.await?;
	tracked.track().await?;
	let project = std::fs::canonicalize(harness.temp_path())?.join("tracked/project");

	let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
	let bound = agent_mcp::serve(harness.core.context.clone(), listener, Limits::default())?;
	let base = format!("http://{bound}");
	let http = reqwest::Client::builder()
		.redirect(Policy::none())
		.build()?;

	// Discovery starts from the 401 on /mcp.
	let response = http
		.post(format!("{base}/mcp"))
		.json_body(&json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }))
		.send()
		.await?;
	assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
	let challenge = response.headers()[header::WWW_AUTHENTICATE]
		.to_str()?
		.to_string();
	let metadata_url = challenge
		.split("resource_metadata=\"")
		.nth(1)
		.and_then(|rest| rest.split('"').next())
		.expect("resource_metadata in the challenge")
		.to_string();
	assert_eq!(
		metadata_url,
		format!("{base}/.well-known/oauth-protected-resource/mcp")
	);

	let resource = body(http.get(&metadata_url).send().await?).await;
	assert_eq!(resource["resource"], format!("{base}/mcp"));
	assert_eq!(resource["bearer_methods_supported"], json!(["header"]));
	let issuer = resource["authorization_servers"][0]
		.as_str()
		.unwrap()
		.to_string();
	assert_eq!(issuer, base);

	let server = body(
		http.get(format!("{issuer}/.well-known/oauth-authorization-server"))
			.send()
			.await?,
	)
	.await;
	assert_eq!(server["issuer"], base);
	assert_eq!(server["code_challenge_methods_supported"], json!(["S256"]));
	assert_eq!(
		server["token_endpoint_auth_methods_supported"],
		json!(["none"])
	);
	assert_eq!(
		server["grant_types_supported"],
		json!(["authorization_code", "refresh_token"])
	);

	// A browser page cannot read metadata or register: no CORS, Origin refused.
	let response = http
		.post(server["registration_endpoint"].as_str().unwrap())
		.header(header::ORIGIN, "https://evil.example")
		.json_body(&json!({ "redirect_uris": [REDIRECT] }))
		.send()
		.await?;
	assert_eq!(response.status(), StatusCode::FORBIDDEN);

	// Unsafe redirects are refused at registration.
	let response = http
		.post(server["registration_endpoint"].as_str().unwrap())
		.json_body(&json!({ "redirect_uris": ["http://evil.example/callback"] }))
		.send()
		.await?;
	assert_eq!(response.status(), StatusCode::BAD_REQUEST);

	// Dynamic client registration.
	let response = http
		.post(server["registration_endpoint"].as_str().unwrap())
		.json_body(&json!({
			"client_name": "OAuth acceptance",
			"redirect_uris": [REDIRECT],
			"grant_types": ["authorization_code", "refresh_token"],
			"response_types": ["code"],
			"token_endpoint_auth_method": "none",
		}))
		.send()
		.await?;
	assert_eq!(response.status(), StatusCode::CREATED);
	let registered = body(response).await;
	let flow = Flow {
		http: http.clone(),
		base: base.clone(),
		client_id: registered["client_id"].as_str().unwrap().to_string(),
	};
	assert_eq!(registered["token_endpoint_auth_method"], "none");

	// The consent form refuses a post from any page but its own.
	let (_, challenge) = pkce();
	let request = flow.consent_page(&challenge, "x").await;
	let response = http
		.post(format!("{base}/oauth/authorize"))
		.header(header::ORIGIN, "https://evil.example")
		.form(&[("request", request.as_str()), ("decision", "approve")])
		.send()
		.await?;
	assert_eq!(response.status(), StatusCode::FORBIDDEN);

	// Denying sends the client access_denied and mints nothing.
	let response = http
		.post(format!("{base}/oauth/authorize"))
		.header(header::ORIGIN, &base)
		.form(&[("request", request.as_str()), ("decision", "deny")])
		.send()
		.await?;
	assert_eq!(response.status(), StatusCode::SEE_OTHER);
	let query = query_of(response.headers()[header::LOCATION].to_str()?);
	assert_eq!(query["error"], "access_denied");
	assert_eq!(query["state"], "x");

	// The code flow, then a tool call with the issued token.
	let first = flow.connect(&project).await;
	assert!(
		first.access.starts_with("sdg_"),
		"the access token is a grant token"
	);
	assert!(first.refresh.starts_with("sdr_"));
	let (status, body) = flow.read_readme(&first.access).await;
	assert_eq!(status, StatusCode::OK, "{body}");
	assert_eq!(
		body["result"]["content"][0]["text"],
		"# Readme\nhello from oauth\n"
	);
	let grant = agent_grant::Entity::find()
		.filter(agent_grant::Column::OauthClient.eq(flow.client_id.as_str()))
		.one(harness.library.db().conn())
		.await?
		.expect("the consent minted a grant");
	assert_eq!(grant.caps, GrantCap::Read.as_str());
	assert_eq!(grant.label.as_deref(), Some("OAuth acceptance (OAuth)"));
	assert_ne!(grant.refresh_hash, None);

	// Refresh rotates both tokens on the same grant.
	let (status, body) = flow.refresh(&first.refresh).await;
	assert_eq!(status, StatusCode::OK, "{body}");
	let second = Tokens {
		access: body["access_token"].as_str().unwrap().to_string(),
		refresh: body["refresh_token"].as_str().unwrap().to_string(),
	};
	assert_ne!(second.access, first.access);
	let (status, _) = flow.read_readme(&first.access).await;
	assert_eq!(
		status,
		StatusCode::UNAUTHORIZED,
		"the old access token is gone"
	);
	let (status, body) = flow.read_readme(&second.access).await;
	assert_eq!(status, StatusCode::OK, "{body}");

	// The owner revokes the grant: the token and the refresh token both die.
	RevokeGrantAction::from_input(RevokeGrantInput { id: grant.uuid })
		.map_err(anyhow::Error::msg)?
		.execute(harness.library.clone(), harness.core.context.clone())
		.await?;
	let (status, _) = flow.read_readme(&second.access).await;
	assert_eq!(status, StatusCode::UNAUTHORIZED, "revoked");
	let (status, body) = flow.refresh(&second.refresh).await;
	assert_eq!(status, StatusCode::BAD_REQUEST);
	assert_eq!(body["error"], "invalid_grant");

	// A wrong verifier fails, and the code is then dead even with the right one.
	let (verifier, challenge) = pkce();
	let code = flow.approve(&project, &challenge).await;
	let (status, body) = flow.exchange(&code, &pkce().0).await;
	assert_eq!(status, StatusCode::BAD_REQUEST);
	assert_eq!(body["error"], "invalid_grant");
	let (status, body) = flow.exchange(&code, &verifier).await;
	assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

	// A replayed, already rotated refresh token revokes its grant.
	let third = flow.connect(&project).await;
	let (status, body) = flow.refresh(&third.refresh).await;
	assert_eq!(status, StatusCode::OK, "{body}");
	let fourth = body["access_token"].as_str().unwrap().to_string();
	let (status, body) = flow.refresh(&third.refresh).await;
	assert_eq!(status, StatusCode::BAD_REQUEST);
	assert_eq!(body["error"], "invalid_grant");
	let (status, _) = flow.read_readme(&fourth).await;
	assert_eq!(status, StatusCode::UNAUTHORIZED, "replay revoked the grant");

	// A client can revoke its own grant (RFC 7009).
	let fifth = flow.connect(&project).await;
	let response = http
		.post(server["revocation_endpoint"].as_str().unwrap())
		.form(&[("token", fifth.refresh.as_str())])
		.send()
		.await?;
	assert_eq!(response.status(), StatusCode::OK);
	let (status, _) = flow.read_readme(&fifth.access).await;
	assert_eq!(status, StatusCode::UNAUTHORIZED);

	// Registrations survive a restart of the endpoint.
	let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
	let bound = agent_mcp::serve(harness.core.context.clone(), listener, Limits::default())?;
	let restarted = Flow {
		http,
		base: format!("http://{bound}"),
		client_id: flow.client_id.clone(),
	};
	let sixth = restarted.connect(&project).await;
	let (status, body) = restarted.read_readme(&sixth.access).await;
	assert_eq!(status, StatusCode::OK, "{body}");

	harness.shutdown().await?;
	Ok(())
}
