//! TLS and non-loopback listening for the agent MCP endpoint
//! (`core::service::agent_mcp`).
//!
//! Serves the endpoint over a real TLS listener on an ephemeral loopback port
//! with a certificate generated at test time, and talks to it with a client
//! that trusts only that certificate. It checks the MCP handshake and a read
//! over TLS, the 401 for a bad token, the 403 for a Host that does not name
//! the listener, OAuth metadata with https URLs on loopback, and that a plain
//! HTTP client gets nothing from the TLS port. When the host has a
//! non-loopback address, it checks that the daemon refuses to listen there
//! without TLS, and that with TLS it serves MCP and REST reads, refuses every
//! write tool with 403 while the same grant writes over loopback, screens Host
//! and answers 404 on every OAuth route. On loopback it checks that a write
//! through a configured host name or with any proxy header gets the same 403.
//! On loopback it checks that a configured host name passes Host but gets no
//! OAuth, and that a request carrying a proxy header gets none either.

#![cfg(unix)]

mod helpers;

use std::{net::IpAddr, path::Path};

use helpers::*;
use reqwest_rustls::{header, StatusCode};
use sd_core::{
	config::AgentMcpConfig,
	infra::action::LibraryAction,
	ops::grants::{
		create::{CreateGrantAction, CreateGrantInput, CreateGrantOutput},
		GrantCap,
	},
	service::agent_mcp::{self, Limits},
};
use serde_json::{json, Value};

async fn grant(
	harness: &IndexingHarness,
	path: &Path,
	caps: GrantCap,
) -> anyhow::Result<CreateGrantOutput> {
	Ok(CreateGrantAction::from_input(CreateGrantInput {
		path: path.to_path_buf(),
		caps: Some(caps),
		ttl_seconds: None,
		label: Some("mcp tls".to_string()),
		kind: None,
		token_format: None,
	})
	.map_err(anyhow::Error::msg)?
	.execute(harness.library.clone(), harness.core.context.clone())
	.await?)
}

struct Reply {
	status: StatusCode,
	headers: header::HeaderMap,
	body: Value,
}

async fn post(
	http: &reqwest_rustls::Client,
	url: &str,
	token: &str,
	session: Option<&str>,
	host: Option<&str>,
	message: Value,
) -> Reply {
	let mut request = http
		.post(url)
		.bearer_auth(token)
		.header(header::ACCEPT, "application/json, text/event-stream")
		.header(header::CONTENT_TYPE, "application/json")
		.body(message.to_string());
	if let Some(session) = session {
		request = request.header("mcp-session-id", session);
	}
	if let Some(host) = host {
		request = request.header(header::HOST, host);
	}
	let response = request.send().await.expect("request");
	let status = response.status();
	let headers = response.headers().clone();
	let bytes = response.bytes().await.expect("body");
	Reply {
		status,
		headers,
		body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
	}
}

fn initialize() -> Value {
	json!({
		"jsonrpc": "2.0",
		"id": 1,
		"method": "initialize",
		"params": {
			"protocolVersion": "2025-06-18",
			"capabilities": {},
			"clientInfo": { "name": "tls", "version": "0" },
		},
	})
}

#[tokio::test]
async fn agent_mcp_serves_grants_over_tls() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("agent_mcp_tls")
		.disable_watcher()
		.build()
		.await?;
	let tracked = harness.create_test_dir("tracked").await?;
	tracked
		.write_file("project/readme.md", "# Over TLS\n")
		.await?;
	tracked.track().await?;
	let project = std::fs::canonicalize(harness.temp_path())?.join("tracked/project");
	let granted = grant(&harness, &project, GrantCap::Read).await?;

	let certified =
		rcgen::generate_simple_self_signed(vec!["localhost".to_string(), "127.0.0.1".to_string()])?;
	let cert_pem = certified.cert.pem();
	let key_pem = certified.signing_key.serialize_pem();
	let tls = agent_mcp::tls_config(cert_pem.as_bytes(), key_pem.as_bytes())?;

	let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
	let bound = agent_mcp::serve_tls(
		harness.core.context.clone(),
		listener,
		Limits::default(),
		tls,
		Vec::new(),
	)?;
	let url = format!("https://{bound}/mcp");
	let http = reqwest_rustls::Client::builder()
		.add_root_certificate(reqwest_rustls::Certificate::from_pem(cert_pem.as_bytes())?)
		.build()?;

	// A bad token gets the same 401 as on plain HTTP, pointing at https metadata.
	let stranger = format!("sdg_{}", "0".repeat(64));
	let reply = post(&http, &url, &stranger, None, None, initialize()).await;
	assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
	let challenge = reply.headers[header::WWW_AUTHENTICATE].to_str()?;
	assert!(
		challenge.contains(&format!("https://{bound}/.well-known/")),
		"{challenge}"
	);

	// A Host that does not name the listener is refused before the token is read.
	let reply = post(
		&http,
		&url,
		&granted.token,
		None,
		Some(&format!("evil.example:{}", bound.port())),
		initialize(),
	)
	.await;
	assert_eq!(reply.status, StatusCode::FORBIDDEN);

	// The handshake, the tool list and a read.
	let reply = post(&http, &url, &granted.token, None, None, initialize()).await;
	assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
	assert_eq!(reply.body["result"]["serverInfo"]["name"], "spacedrive");
	let session = reply.headers["mcp-session-id"].to_str()?.to_string();
	let reply = post(
		&http,
		&url,
		&granted.token,
		Some(&session),
		None,
		json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
	)
	.await;
	assert_eq!(reply.status, StatusCode::ACCEPTED);
	let reply = post(
		&http,
		&url,
		&granted.token,
		Some(&session),
		None,
		json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
	)
	.await;
	assert_eq!(reply.status, StatusCode::OK);
	let tools = reply.body["result"]["tools"].as_array().expect("tools");
	assert!(tools.iter().any(|tool| tool["name"] == "read"));
	let reply = post(
		&http,
		&url,
		&granted.token,
		Some(&session),
		None,
		json!({
			"jsonrpc": "2.0",
			"id": 3,
			"method": "tools/call",
			"params": { "name": "read", "arguments": { "path": "readme.md" } },
		}),
	)
	.await;
	assert_eq!(reply.status, StatusCode::OK);
	let result = &reply.body["result"];
	assert_eq!(result["isError"], json!(false), "{result}");
	assert!(result.to_string().contains("# Over TLS"), "{result}");

	// OAuth is offered on a loopback TLS listener, with an https issuer.
	let metadata: Value = http
		.get(format!(
			"https://{bound}/.well-known/oauth-authorization-server"
		))
		.send()
		.await?
		.json()
		.await?;
	assert_eq!(metadata["issuer"], format!("https://{bound}"));

	// Plain HTTP to the TLS port gets no answer.
	let plain = reqwest_rustls::Client::new()
		.post(format!("http://{bound}/mcp"))
		.bearer_auth(&granted.token)
		.body(initialize().to_string())
		.send()
		.await;
	assert!(plain.is_err(), "plain HTTP was answered: {plain:?}");

	harness.shutdown().await?;
	Ok(())
}

/// The address this host routes outward from, found without sending a
/// packet, or None on a host with only loopback.
fn local_non_loopback_ip() -> Option<IpAddr> {
	let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
	socket.connect("192.0.2.1:9").ok()?;
	let ip = socket.local_addr().ok()?.ip();
	(!ip.is_loopback() && !ip.is_unspecified()).then_some(ip)
}

/// Every OAuth route on `origin`, which must all answer 404 off loopback.
async fn assert_no_oauth(http: &reqwest_rustls::Client, origin: &str) {
	for path in [
		"/.well-known/oauth-protected-resource",
		"/.well-known/oauth-protected-resource/mcp",
		"/.well-known/oauth-authorization-server",
		"/oauth/authorize",
	] {
		let status = http
			.get(format!("{origin}{path}"))
			.send()
			.await
			.expect("request")
			.status();
		assert_eq!(status, StatusCode::NOT_FOUND, "GET {path}");
	}
	for path in [
		"/oauth/register",
		"/oauth/authorize",
		"/oauth/token",
		"/oauth/revoke",
	] {
		let status = http
			.post(format!("{origin}{path}"))
			.body("{}")
			.send()
			.await
			.expect("request")
			.status();
		assert_eq!(status, StatusCode::NOT_FOUND, "POST {path}");
	}
}

/// The status and `WWW-Authenticate` value of an initialize with an unknown
/// token and `headers`.
async fn challenge(
	http: &reqwest_rustls::Client,
	url: &str,
	headers: &[(&str, String)],
) -> (StatusCode, String) {
	let mut request = http
		.post(url)
		.bearer_auth(format!("sdg_{}", "0".repeat(64)))
		.header(header::CONTENT_TYPE, "application/json")
		.body(initialize().to_string());
	for (name, value) in headers {
		request = request.header(*name, value);
	}
	let response = request.send().await.expect("request");
	let value = response
		.headers()
		.get(header::WWW_AUTHENTICATE)
		.map(|v| v.to_str().unwrap().to_string())
		.unwrap_or_default();
	(response.status(), value)
}

/// The status of a GET of the authorization server metadata with `headers`.
async fn metadata_status(
	http: &reqwest_rustls::Client,
	origin: &str,
	headers: &[(&str, String)],
) -> StatusCode {
	let mut request = http.get(format!("{origin}/.well-known/oauth-authorization-server"));
	for (name, value) in headers {
		request = request.header(*name, value);
	}
	request.send().await.expect("request").status()
}

/// A `tools/call` of `name` with `arguments` in `session`.
fn tool_call(id: u64, name: &str, arguments: Value) -> Value {
	json!({
		"jsonrpc": "2.0",
		"id": id,
		"method": "tools/call",
		"params": { "name": name, "arguments": arguments },
	})
}

/// An MCP session on `url`: initialize, then the initialized notification.
async fn open_session(http: &reqwest_rustls::Client, url: &str, token: &str) -> String {
	let reply = post(http, url, token, None, None, initialize()).await;
	assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
	assert_eq!(reply.body["result"]["serverInfo"]["name"], "spacedrive");
	let session = reply.headers["mcp-session-id"]
		.to_str()
		.expect("session id")
		.to_string();
	let reply = post(
		http,
		url,
		token,
		Some(&session),
		None,
		json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
	)
	.await;
	assert_eq!(reply.status, StatusCode::ACCEPTED);
	session
}

/// A REST tool call of `name` on `space` at `origin`, with extra `headers`.
async fn rest_tool(
	http: &reqwest_rustls::Client,
	origin: &str,
	token: &str,
	space: &str,
	name: &str,
	arguments: Value,
	headers: &[(&str, String)],
) -> (StatusCode, Value) {
	let mut request = http
		.post(format!("{origin}/v1/spaces/{space}/tools/{name}"))
		.bearer_auth(token)
		.header(header::CONTENT_TYPE, "application/json")
		.body(arguments.to_string());
	for (name, value) in headers {
		request = request.header(*name, value);
	}
	let response = request.send().await.expect("request");
	let status = response.status();
	let body = response.json().await.unwrap_or(Value::Null);
	(status, body)
}

const NETWORK_WRITES_OFF: &str = "writes over the network are off until the security review";

#[tokio::test]
async fn non_loopback_listeners_need_tls_and_serve_reads_only() -> anyhow::Result<()> {
	let Some(ip) = local_non_loopback_ip() else {
		eprintln!("skipping: this host has no non-loopback address to listen on");
		return Ok(());
	};
	let harness = IndexingHarnessBuilder::new("agent_mcp_non_loopback")
		.disable_watcher()
		.build()
		.await?;
	let tracked = harness.create_test_dir("tracked").await?;
	tracked
		.write_file("project/readme.md", "# Over the network\n")
		.await?;
	tracked.track().await?;
	let project = std::fs::canonicalize(harness.temp_path())?.join("tracked/project");
	let granted = grant(&harness, &project, GrantCap::ReadWrite).await?;
	let token = granted.token.as_str();
	let space = granted.grant.id.to_string();
	let listen = std::net::SocketAddr::new(ip, 0).to_string();

	// Without TLS the daemon refuses the address.
	let plain = AgentMcpConfig {
		enabled: true,
		listen: listen.clone(),
		..AgentMcpConfig::default()
	};
	let refused = agent_mcp::start(harness.core.context.clone(), &plain)
		.await
		.unwrap_err();
	assert!(refused.to_string().contains("tls_cert"), "{refused}");

	// With TLS files it starts, and a client that trusts only that
	// certificate completes the handshake.
	let certified =
		rcgen::generate_simple_self_signed(vec![ip.to_string(), "nas.example.test".to_string()])?;
	let cert_pem = certified.cert.pem();
	let cert_path = harness.temp_path().join("agent-mcp-cert.pem");
	let key_path = harness.temp_path().join("agent-mcp-key.pem");
	std::fs::write(&cert_path, &cert_pem)?;
	std::fs::write(&key_path, certified.signing_key.serialize_pem())?;
	let config = AgentMcpConfig {
		enabled: true,
		listen,
		tls_cert: Some(cert_path),
		tls_key: Some(key_path),
		host_names: vec!["nas.example.test".to_string()],
		..AgentMcpConfig::default()
	};
	let bound = agent_mcp::start(harness.core.context.clone(), &config).await?;
	assert_eq!(bound.ip(), ip);
	let https = reqwest_rustls::Client::builder()
		.add_root_certificate(reqwest_rustls::Certificate::from_pem(cert_pem.as_bytes())?)
		.build()?;
	let origin = format!("https://{bound}");
	let url = format!("{origin}/mcp");

	// MCP: the handshake and a read work.
	let session = open_session(&https, &url, token).await;
	let reply = post(
		&https,
		&url,
		token,
		Some(&session),
		None,
		tool_call(2, "read", json!({ "path": "readme.md" })),
	)
	.await;
	assert_eq!(reply.status, StatusCode::OK);
	let result = &reply.body["result"];
	assert_eq!(result["isError"], json!(false), "{result}");
	assert!(
		result.to_string().contains("# Over the network"),
		"{result}"
	);

	// MCP: a write tool is refused before it runs, even on a writable grant.
	let reply = post(
		&https,
		&url,
		token,
		Some(&session),
		None,
		tool_call(
			3,
			"write_file",
			json!({ "path": "mcp.txt", "content": "x" }),
		),
	)
	.await;
	assert_eq!(reply.status, StatusCode::FORBIDDEN, "{}", reply.body);
	assert_eq!(reply.body["error"]["message"], NETWORK_WRITES_OFF);
	assert!(!project.join("mcp.txt").exists());

	// REST: the space list and a block read work.
	let response = https
		.get(format!("{origin}/v1/spaces"))
		.bearer_auth(token)
		.send()
		.await?;
	assert_eq!(response.status(), StatusCode::OK);
	let spaces: Value = response.json().await?;
	assert!(spaces.to_string().contains(&space), "{spaces}");
	let response = https
		.get(format!("{origin}/v1/spaces/{space}/blocks/readme.md"))
		.bearer_auth(token)
		.header(header::RANGE, "bytes=0-1")
		.send()
		.await?;
	assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
	assert_eq!(&response.bytes().await?[..], b"# ");

	// REST: a write tool is refused the same way.
	let (status, body) = rest_tool(
		&https,
		&origin,
		token,
		&space,
		"write_file",
		json!({ "path": "rest.txt", "content": "x" }),
		&[],
	)
	.await;
	assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
	assert_eq!(body["error"]["message"], NETWORK_WRITES_OFF);
	assert!(!project.join("rest.txt").exists());

	// Host must name the listener, and no OAuth route answers.
	for host in [
		bound.to_string(),
		format!("nas.example.test:{}", bound.port()),
	] {
		let (status, value) = challenge(&https, &url, &[("host", host.clone())]).await;
		assert_eq!(status, StatusCode::UNAUTHORIZED, "{host}");
		assert!(!value.contains("resource_metadata"), "{value}");
	}
	for host in [
		format!("localhost:{}", bound.port()),
		format!("evil.example:{}", bound.port()),
		"nas.example.test".to_string(),
	] {
		let (status, _) = challenge(&https, &url, &[("host", host.clone())]).await;
		assert_eq!(status, StatusCode::FORBIDDEN, "{host}");
	}
	assert_no_oauth(&https, &origin).await;

	// The same grant on a loopback listener keeps read-write.
	let loopback = AgentMcpConfig {
		enabled: true,
		listen: "127.0.0.1:0".to_string(),
		..AgentMcpConfig::default()
	};
	let local = agent_mcp::start(harness.core.context.clone(), &loopback).await?;
	let http = reqwest_rustls::Client::new();
	let origin = format!("http://{local}");
	let url = format!("{origin}/mcp");
	let session = open_session(&http, &url, token).await;
	let reply = post(
		&http,
		&url,
		token,
		Some(&session),
		None,
		tool_call(
			2,
			"write_file",
			json!({ "path": "mcp.txt", "content": "x" }),
		),
	)
	.await;
	assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
	assert_eq!(
		reply.body["result"]["isError"],
		json!(false),
		"{}",
		reply.body
	);
	assert!(project.join("mcp.txt").exists());
	let (status, body) = rest_tool(
		&http,
		&origin,
		token,
		&space,
		"write_file",
		json!({ "path": "rest.txt", "content": "x" }),
		&[],
	)
	.await;
	assert_eq!(status, StatusCode::OK, "{body}");
	assert!(project.join("rest.txt").exists());

	harness.shutdown().await?;
	Ok(())
}

#[tokio::test]
async fn oauth_is_offered_only_to_requests_straight_from_this_machine() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("agent_mcp_proxied")
		.disable_watcher()
		.build()
		.await?;
	let config = AgentMcpConfig {
		enabled: true,
		listen: "127.0.0.1:0".to_string(),
		host_names: vec!["nas.example.test".to_string()],
		..AgentMcpConfig::default()
	};
	let bound = agent_mcp::start(harness.core.context.clone(), &config).await?;
	let http = reqwest_rustls::Client::new();
	let origin = format!("http://{bound}");
	let url = format!("{origin}/mcp");

	// Straight from this machine: OAuth is offered.
	let (status, value) = challenge(&http, &url, &[]).await;
	assert_eq!(status, StatusCode::UNAUTHORIZED);
	assert!(value.contains("resource_metadata"), "{value}");
	assert_eq!(metadata_status(&http, &origin, &[]).await, StatusCode::OK);

	// A configured host name on a loopback bind passes the Host check, with
	// any port, so a proxy that keeps Host works, but it gets no OAuth.
	for host in ["nas.example.test", "nas.example.test:443"] {
		let headers = [("host", host.to_string())];
		let (status, value) = challenge(&http, &url, &headers).await;
		assert_eq!(status, StatusCode::UNAUTHORIZED, "{host}");
		assert!(!value.contains("resource_metadata"), "{value}");
		assert_eq!(
			metadata_status(&http, &origin, &headers).await,
			StatusCode::NOT_FOUND,
			"{host}"
		);
	}
	let (status, _) = challenge(&http, &url, &[("host", "evil.example".to_string())]).await;
	assert_eq!(status, StatusCode::FORBIDDEN);

	// The REST surface screens Host the same way and never points at OAuth.
	for (host, expected) in [
		("nas.example.test".to_string(), StatusCode::UNAUTHORIZED),
		(
			format!("127.0.0.1:{}", bound.port()),
			StatusCode::UNAUTHORIZED,
		),
		("evil.example".to_string(), StatusCode::FORBIDDEN),
	] {
		let response = http
			.get(format!("{origin}/v1/spaces"))
			.header("host", &host)
			.bearer_auth(format!("sdg_{}", "0".repeat(64)))
			.send()
			.await?;
		assert_eq!(response.status(), expected, "{host}");
		let value = response
			.headers()
			.get(header::WWW_AUTHENTICATE)
			.map(|v| v.to_str().unwrap().to_string())
			.unwrap_or_default();
		assert!(!value.contains("resource_metadata"), "{host}: {value}");
	}

	// A request a proxy forwarded gets no OAuth, even with a loopback Host.
	for name in [
		"forwarded",
		"x-forwarded-for",
		"x-forwarded-host",
		"x-real-ip",
	] {
		let headers = [(name, "203.0.113.7".to_string())];
		let (status, value) = challenge(&http, &url, &headers).await;
		assert_eq!(status, StatusCode::UNAUTHORIZED, "{name}");
		assert!(!value.contains("resource_metadata"), "{name}: {value}");
		assert_eq!(
			metadata_status(&http, &origin, &headers).await,
			StatusCode::NOT_FOUND,
			"{name}"
		);
		let status = http
			.post(format!("{origin}/oauth/register"))
			.header(name, "203.0.113.7")
			.body("{}")
			.send()
			.await?
			.status();
		assert_eq!(status, StatusCode::NOT_FOUND, "{name}");
	}

	harness.shutdown().await?;
	Ok(())
}

/// An MCP `write_file` of `path` in `session`, with extra `headers`.
async fn mcp_write(
	http: &reqwest_rustls::Client,
	url: &str,
	token: &str,
	session: &str,
	path: &str,
	headers: &[(&str, String)],
) -> (StatusCode, Value) {
	let mut request = http
		.post(url)
		.bearer_auth(token)
		.header(header::ACCEPT, "application/json, text/event-stream")
		.header(header::CONTENT_TYPE, "application/json")
		.header("mcp-session-id", session)
		.body(tool_call(9, "write_file", json!({ "path": path, "content": "x" })).to_string());
	for (name, value) in headers {
		request = request.header(*name, value);
	}
	let response = request.send().await.expect("request");
	let status = response.status();
	(status, response.json().await.unwrap_or(Value::Null))
}

#[tokio::test]
async fn writes_need_a_direct_local_request() -> anyhow::Result<()> {
	let harness = IndexingHarnessBuilder::new("agent_mcp_local_writes")
		.disable_watcher()
		.build()
		.await?;
	let tracked = harness.create_test_dir("tracked").await?;
	tracked.write_file("project/readme.md", "# Local\n").await?;
	tracked.track().await?;
	let project = std::fs::canonicalize(harness.temp_path())?.join("tracked/project");
	let granted = grant(&harness, &project, GrantCap::ReadWrite).await?;
	let token = granted.token.as_str();
	let space = granted.grant.id.to_string();
	let config = AgentMcpConfig {
		enabled: true,
		listen: "127.0.0.1:0".to_string(),
		host_names: vec!["nas.example.test".to_string()],
		..AgentMcpConfig::default()
	};
	let bound = agent_mcp::start(harness.core.context.clone(), &config).await?;
	let http = reqwest_rustls::Client::new();
	let origin = format!("http://{bound}");
	let url = format!("{origin}/mcp");
	let session = open_session(&http, &url, token).await;

	// A configured host name, or any proxy header, means the request may have
	// come from another machine, so a write is refused on both surfaces.
	let mut refused: Vec<(&str, String)> = vec![("host", "nas.example.test".to_string())];
	for name in [
		"forwarded",
		"x-forwarded-for",
		"x-forwarded-host",
		"x-real-ip",
	] {
		refused.push((name, "203.0.113.7".to_string()));
	}
	for (index, header) in refused.iter().enumerate() {
		let headers = [header.clone()];
		let path = format!("mcp-{index}.txt");
		let (status, body) = mcp_write(&http, &url, token, &session, &path, &headers).await;
		assert_eq!(status, StatusCode::FORBIDDEN, "{}: {body}", header.0);
		assert_eq!(body["error"]["message"], NETWORK_WRITES_OFF, "{}", header.0);
		assert!(!project.join(&path).exists(), "{}", header.0);

		let path = format!("rest-{index}.txt");
		let (status, body) = rest_tool(
			&http,
			&origin,
			token,
			&space,
			"write_file",
			json!({ "path": path, "content": "x" }),
			&headers,
		)
		.await;
		assert_eq!(status, StatusCode::FORBIDDEN, "{}: {body}", header.0);
		assert_eq!(body["error"]["message"], NETWORK_WRITES_OFF, "{}", header.0);
		assert!(!project.join(&path).exists(), "{}", header.0);
	}

	// Reads still work through the proxy name.
	let (status, body) = rest_tool(
		&http,
		&origin,
		token,
		&space,
		"read",
		json!({ "path": "readme.md" }),
		&[("host", "nas.example.test".to_string())],
	)
	.await;
	assert_eq!(status, StatusCode::OK, "{body}");

	// Straight from this machine, the same writes succeed.
	let (status, body) = mcp_write(&http, &url, token, &session, "mcp.txt", &[]).await;
	assert_eq!(status, StatusCode::OK, "{body}");
	assert_eq!(body["result"]["isError"], json!(false), "{body}");
	assert!(project.join("mcp.txt").exists());
	let (status, body) = rest_tool(
		&http,
		&origin,
		token,
		&space,
		"write_file",
		json!({ "path": "rest.txt", "content": "x" }),
		&[],
	)
	.await;
	assert_eq!(status, StatusCode::OK, "{body}");
	assert!(project.join("rest.txt").exists());

	harness.shutdown().await?;
	Ok(())
}
