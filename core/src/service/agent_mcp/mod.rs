//! # Agent MCP endpoint
//!
//! `core::service::agent_mcp` serves the holder of an agent grant a small,
//! read-only tool set over MCP's streamable HTTP transport. An agent points
//! its MCP client at `http://127.0.0.1:7765/mcp` and sends the grant token as
//! `Authorization: Bearer sdg_...`. It can then list, stat, read, glob, grep
//! and search inside the granted folder, and nothing else.
//!
//! The endpoint is a separate listener. It never forwards to the RPC socket
//! and has no route into the op registry, so a grant cannot reach actions,
//! remote ops or another library. Every request goes through
//! [`crate::ops::grants::authorize`], which reads the grant row each time, so
//! a revoked or expired grant fails its next request. The endpoint then opens
//! only [`crate::ops::grants::GrantScope::path`], never the path the client
//! sent.
//!
//! The transport is the subset of streamable HTTP a tool-only server needs,
//! written by hand: one POST per JSON-RPC message, answered with
//! `application/json`, a session id minted at `initialize`, and DELETE to end
//! a session. The server never opens an SSE stream, so GET answers 405 as the
//! spec allows. That keeps authentication, session binding and limits in one
//! handler instead of spread across a framework's layers.
//!
//! It is off by default and binds loopback. It can serve TLS from a configured
//! certificate and key. The config also takes a non-loopback IP with TLS or an
//! explicit `plaintext` flag, but [`transport`] refuses every non-loopback
//! address until reads go through handle-based path containment; wildcard
//! addresses are always refused. Host must name the listener: a loopback name
//! with the bound port, the bound IP, or a configured host name for a proxy.
//! OAuth is offered only to a request that comes straight from this machine,
//! since its consent page mints a grant for whoever presses Allow without
//! authenticating the owner. Everywhere else every OAuth route answers 404 and
//! clients use a token from `sd grant`. The checklist that governs this is in
//! `docs/core/design/agent-access-threat-model.md`.
//!
//! ## Example
//! ```rust,no_run
//! # async fn demo(context: std::sync::Arc<sd_core::context::CoreContext>) -> anyhow::Result<()> {
//! let config = sd_core::config::AgentMcpConfig::default();
//! let bound = sd_core::service::agent_mcp::start(context, &config).await?;
//! assert!(bound.ip().is_loopback());
//! # Ok(())
//! # }
//! ```

mod events;
#[cfg(feature = "footage-clips")]
mod footage;
mod http;
mod limits;
mod oauth;
mod paths;
mod rest;
mod tools;

use std::{
	net::{IpAddr, Ipv4Addr, SocketAddr},
	path::PathBuf,
	sync::Arc,
	time::Duration,
};

use axum::{extract::ConnectInfo, Extension, Router};
use hyper_util::{
	rt::{TokioIo, TokioTimer},
	service::TowerToHyperService,
};
use rustls::{
	pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer},
	ServerConfig,
};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tracing::{debug, error, info, warn};

use crate::{config::AgentMcpConfig, context::CoreContext};

pub use limits::Limits;

/// The port the endpoint listens on unless the config names another.
pub const DEFAULT_PORT: u16 = 7765;
/// The default value of `agent_mcp.listen`.
pub const DEFAULT_LISTEN: &str = "127.0.0.1:7765";
/// The URL path MCP clients connect to.
pub const MCP_PATH: &str = "/mcp";
/// The skill `get_skill` returns. It is compiled in, never read from a grant.
pub const SKILL: &str = include_str!("SKILL.md");

/// How long a client has to finish the TLS handshake, and later to send a
/// request's headers, before its connection is dropped.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

const NO_CONTAINMENT: &str = "the agent MCP endpoint does not listen beyond loopback yet. That needs handle-based path containment (docs/core/design/agent-access-threat-model.md, item 9), which this build does not have. Keep it on loopback behind a proxy that forwards only /mcp";

const NON_LOOPBACK_PLAIN: &str = "a non-loopback listener needs agent_mcp.tls_cert and agent_mcp.tls_key, or agent_mcp.plaintext set to true for a network that is already encrypted";

/// The socket address `listen` names, or why the endpoint will not bind it.
///
/// Accepts an IP with or without a port, such as `127.0.0.1:7765`,
/// `[::1]:7765` or `100.64.0.1`, and `localhost` with or without a port.
/// A missing port gets [`DEFAULT_PORT`]. Wildcards and every other host name
/// are refused, since a name could resolve to an address the user did not
/// mean to expose.
pub fn listen_addr(listen: &str) -> Result<SocketAddr, String> {
	let text = listen.trim();
	let localhost = |port: u16| SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
	let addr = if let Ok(addr) = text.parse::<SocketAddr>() {
		addr
	} else if let Ok(ip) = text.parse::<IpAddr>() {
		SocketAddr::new(ip, DEFAULT_PORT)
	} else if text == "localhost" {
		localhost(DEFAULT_PORT)
	} else if let Some(port) = text.strip_prefix("localhost:") {
		localhost(
			port.parse()
				.map_err(|_| format!("'{listen}' does not name a port"))?,
		)
	} else {
		return Err(format!(
			"'{listen}' is not an IP address such as {DEFAULT_LISTEN}"
		));
	};
	if addr.ip().is_unspecified() {
		return Err(format!(
			"'{listen}' is a wildcard address; the agent MCP endpoint binds one explicit address"
		));
	}
	Ok(addr)
}

/// Why `name` cannot be a host name the endpoint accepts in Host, if it
/// cannot. Only DNS names are taken; the bound IP is always accepted.
pub fn host_name(name: &str) -> Result<(), String> {
	let valid = !name.is_empty()
		&& name.len() <= 253
		&& name.split('.').all(|label| {
			!label.is_empty()
				&& label.len() <= 63
				&& !label.starts_with('-')
				&& !label.ends_with('-')
				&& label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
		});
	if valid {
		Ok(())
	} else {
		Err(format!(
			"'{name}' is not a host name such as nas.example.ts.net"
		))
	}
}

/// How the endpoint serves its listen address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transport {
	/// Plain HTTP on a loopback address.
	Loopback,
	/// TLS from the configured certificate chain and key.
	Tls { cert: PathBuf, key: PathBuf },
	/// Plain HTTP beyond loopback, which the user asked for with `plaintext`.
	Plaintext,
}

/// Decide how to serve `addr` under `config`, or why it must not be served.
///
/// A configured certificate and key always mean TLS, on loopback too, so the
/// URL the user hands out matches what the listener speaks.
pub fn transport(addr: SocketAddr, config: &AgentMcpConfig) -> Result<Transport, String> {
	for name in &config.host_names {
		host_name(name)?;
	}
	let tls = match (&config.tls_cert, &config.tls_key) {
		(Some(cert), Some(key)) => Some(Transport::Tls {
			cert: cert.clone(),
			key: key.clone(),
		}),
		(None, None) => None,
		_ => return Err("set both agent_mcp.tls_cert and agent_mcp.tls_key, or neither".into()),
	};
	if !addr.ip().is_loopback() {
		return Err(format!("refusing to listen on {addr}: {NO_CONTAINMENT}"));
	}
	if let Some(tls) = tls {
		return Ok(tls);
	}
	if addr.ip().is_loopback() {
		Ok(Transport::Loopback)
	} else if config.plaintext {
		Ok(Transport::Plaintext)
	} else {
		Err(format!(
			"refusing to listen on {addr}: {NON_LOOPBACK_PLAIN}"
		))
	}
}

/// The URL MCP clients connect to for an endpoint listening on `listen`.
///
/// A non-loopback listener is reached by its first host name when it has
/// one, since that is the name a certificate is usually issued for.
pub fn url(listen: &str, tls: bool, host_names: &[String]) -> String {
	let addr = listen_addr(listen).unwrap_or_else(|_| {
		DEFAULT_LISTEN
			.parse()
			.expect("the default listen address parses")
	});
	let scheme = if tls { "https" } else { "http" };
	let authority = match host_names.first() {
		Some(name) if !addr.ip().is_loopback() => format!("{name}:{}", addr.port()),
		_ => addr.to_string(),
	};
	format!("{scheme}://{authority}{MCP_PATH}")
}

/// A rustls server config for TLS 1.2 and 1.3 from a PEM certificate chain
/// and a PEM private key.
pub fn tls_config(cert_pem: &[u8], key_pem: &[u8]) -> anyhow::Result<Arc<ServerConfig>> {
	let certs = CertificateDer::pem_slice_iter(cert_pem)
		.collect::<Result<Vec<_>, _>>()
		.map_err(|e| anyhow::anyhow!("the TLS certificate is not valid PEM: {e}"))?;
	if certs.is_empty() {
		anyhow::bail!("the TLS certificate file holds no certificate");
	}
	let key = PrivateKeyDer::from_pem_slice(key_pem)
		.map_err(|e| anyhow::anyhow!("the TLS key is not a valid PEM private key: {e}"))?;
	// Name the provider, since more than one is compiled into the daemon.
	let mut config =
		ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
			.with_protocol_versions(&[&rustls::version::TLS13, &rustls::version::TLS12])?
			.with_no_client_auth()
			.with_single_cert(certs, key)?;
	config.alpn_protocols = vec![b"http/1.1".to_vec()];
	Ok(Arc::new(config))
}

/// Bind the endpoint as `config` describes and serve it for the life of the
/// process.
///
/// A refused address or an unreadable certificate is logged with the reason
/// and returned as an error, so the daemon keeps running without the
/// endpoint.
pub async fn start(
	context: Arc<CoreContext>,
	config: &AgentMcpConfig,
) -> anyhow::Result<SocketAddr> {
	let refuse = |reason: String| {
		error!("Agent MCP endpoint not started: {reason}");
		anyhow::anyhow!(reason)
	};
	let addr = listen_addr(&config.listen).map_err(refuse)?;
	let transport = transport(addr, config).map_err(refuse)?;
	let tls = match &transport {
		Transport::Tls { cert, key } => {
			let cert = tokio::fs::read(cert)
				.await
				.map_err(|e| refuse(format!("cannot read {}: {e}", cert.display())))?;
			let key = tokio::fs::read(key)
				.await
				.map_err(|e| refuse(format!("cannot read {}: {e}", key.display())))?;
			Some(tls_config(&cert, &key).map_err(|e| refuse(e.to_string()))?)
		}
		_ => None,
	};
	let listener = TcpListener::bind(addr).await?;
	let host_names = config.host_names.clone();
	let bound = match tls {
		Some(tls) => serve_tls(context, listener, Limits::default(), tls, host_names)?,
		None => spawn_plain(context, listener, Limits::default(), host_names)?,
	};
	if transport == Transport::Plaintext {
		warn!(
			"Agent MCP endpoint serves plain HTTP on {bound}. Grant tokens cross the network unencrypted unless the network itself is encrypted"
		);
	}
	info!(
		"Agent MCP endpoint at {}",
		url(&bound.to_string(), tls_on(&transport), &config.host_names)
	);
	Ok(bound)
}

fn tls_on(transport: &Transport) -> bool {
	matches!(transport, Transport::Tls { .. })
}

/// Serve the endpoint over plain HTTP on a listener that is already bound to
/// loopback.
///
/// Tests bind an ephemeral port and pass lower limits through `limits`.
pub fn serve(
	context: Arc<CoreContext>,
	listener: TcpListener,
	limits: Limits,
) -> anyhow::Result<SocketAddr> {
	let bound = listener.local_addr()?;
	if !bound.ip().is_loopback() {
		anyhow::bail!("refusing to serve plain HTTP on {bound}: {NON_LOOPBACK_PLAIN}");
	}
	spawn_plain(context, listener, limits, Vec::new())
}

fn spawn_plain(
	context: Arc<CoreContext>,
	listener: TcpListener,
	limits: Limits,
	host_names: Vec<String>,
) -> anyhow::Result<SocketAddr> {
	let bound = listener.local_addr()?;
	let endpoint = Arc::new(http::Endpoint::new(
		context, bound, limits, false, host_names,
	));
	let app = http::router(endpoint).into_make_service_with_connect_info::<SocketAddr>();
	tokio::spawn(async move {
		if let Err(err) = axum::serve(listener, app).await {
			error!("Agent MCP endpoint exited: {err}");
		}
	});
	Ok(bound)
}

/// Serve the endpoint over TLS on a listener bound to any explicit address,
/// loopback included.
///
/// `host_names` are the names a non-loopback listener accepts in Host besides
/// its IP. Tests pass a config built by [`tls_config`] from a generated
/// certificate.
pub fn serve_tls(
	context: Arc<CoreContext>,
	listener: TcpListener,
	limits: Limits,
	tls: Arc<ServerConfig>,
	host_names: Vec<String>,
) -> anyhow::Result<SocketAddr> {
	let bound = listener.local_addr()?;
	if bound.ip().is_unspecified() {
		anyhow::bail!("refusing to serve on the wildcard address {bound}");
	}
	let endpoint = Arc::new(http::Endpoint::new(
		context, bound, limits, true, host_names,
	));
	let app = http::router(endpoint);
	tokio::spawn(accept_tls(listener, TlsAcceptor::from(tls), app));
	Ok(bound)
}

async fn accept_tls(listener: TcpListener, acceptor: TlsAcceptor, app: Router) {
	loop {
		let (stream, peer) = match listener.accept().await {
			Ok(connection) => connection,
			Err(err) => {
				// Usually out of file descriptors; back off instead of spinning.
				warn!(error = %err, "agent MCP endpoint could not accept a connection");
				tokio::time::sleep(Duration::from_millis(100)).await;
				continue;
			}
		};
		let acceptor = acceptor.clone();
		let app = app.clone();
		tokio::spawn(async move {
			let stream =
				match tokio::time::timeout(HANDSHAKE_TIMEOUT, acceptor.accept(stream)).await {
					Ok(Ok(stream)) => stream,
					Ok(Err(err)) => {
						debug!(%peer, error = %err, "agent MCP TLS handshake failed");
						return;
					}
					Err(_) => {
						debug!(%peer, "agent MCP TLS handshake timed out");
						return;
					}
				};
			let service = TowerToHyperService::new(app.layer(Extension(ConnectInfo(peer))));
			if let Err(err) = hyper::server::conn::http1::Builder::new()
				.timer(TokioTimer::new())
				.header_read_timeout(HANDSHAKE_TIMEOUT)
				.serve_connection(TokioIo::new(stream), service)
				.await
			{
				debug!(%peer, error = %err, "agent MCP TLS connection ended with an error");
			}
		});
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn config(listen: &str) -> AgentMcpConfig {
		AgentMcpConfig {
			enabled: true,
			listen: listen.to_string(),
			..AgentMcpConfig::default()
		}
	}

	fn plan(config: &AgentMcpConfig) -> Result<Transport, String> {
		transport(listen_addr(&config.listen)?, config)
	}

	#[test]
	fn explicit_addresses_are_accepted() {
		assert_eq!(listen_addr(DEFAULT_LISTEN).unwrap().port(), DEFAULT_PORT);
		assert_eq!(listen_addr("127.0.0.1").unwrap().port(), DEFAULT_PORT);
		assert_eq!(listen_addr("[::1]:9000").unwrap().port(), 9000);
		assert_eq!(
			listen_addr("localhost:9001").unwrap(),
			"127.0.0.1:9001".parse::<SocketAddr>().unwrap()
		);
		assert_eq!(
			listen_addr("100.64.0.1").unwrap(),
			"100.64.0.1:7765".parse::<SocketAddr>().unwrap()
		);
		assert_eq!(listen_addr("[fd7a::1]:8443").unwrap().port(), 8443);
	}

	#[test]
	fn wildcards_and_names_are_refused() {
		for listen in [
			"0.0.0.0:7765",
			"0.0.0.0",
			"[::]:7765",
			"::",
			"example.com:7765",
			"nas.local",
			"localhost:port",
		] {
			assert!(listen_addr(listen).is_err(), "{listen} should be refused");
		}
	}

	#[test]
	fn loopback_serves_plain_http_by_default() {
		assert_eq!(plan(&config(DEFAULT_LISTEN)), Ok(Transport::Loopback));
		assert_eq!(plan(&config("[::1]:7765")), Ok(Transport::Loopback));
	}

	#[test]
	fn non_loopback_addresses_are_refused_until_containment_lands() {
		let tls = AgentMcpConfig {
			tls_cert: Some("cert.pem".into()),
			tls_key: Some("key.pem".into()),
			..config("100.64.0.1:7765")
		};
		let plaintext = AgentMcpConfig {
			plaintext: true,
			..config("100.64.0.1:7765")
		};
		for config in [config("192.168.1.5:7765"), tls, plaintext] {
			let refused = plan(&config).unwrap_err();
			assert!(refused.contains("containment"), "{refused}");
		}
	}

	#[test]
	fn a_certificate_and_key_mean_tls_on_loopback() {
		for listen in [DEFAULT_LISTEN, "[::1]:7765"] {
			let config = AgentMcpConfig {
				tls_cert: Some("cert.pem".into()),
				tls_key: Some("key.pem".into()),
				..config(listen)
			};
			assert_eq!(
				plan(&config),
				Ok(Transport::Tls {
					cert: "cert.pem".into(),
					key: "key.pem".into()
				})
			);
		}
	}

	#[test]
	fn half_a_tls_config_is_refused() {
		let config = AgentMcpConfig {
			tls_cert: Some("cert.pem".into()),
			..config(DEFAULT_LISTEN)
		};
		assert!(plan(&config).is_err());
	}

	#[test]
	fn host_names_must_be_dns_names() {
		assert!(host_name("nas.example.ts.net").is_ok());
		assert!(host_name("localhost").is_ok());
		for name in [
			"",
			"nas.example:443",
			"*.example",
			"-bad.example",
			"a..b",
			"a b",
		] {
			assert!(host_name(name).is_err(), "{name:?} should be refused");
		}
		let config = AgentMcpConfig {
			plaintext: true,
			host_names: vec!["bad name".into()],
			..config(DEFAULT_LISTEN)
		};
		assert!(plan(&config).is_err());
	}

	#[test]
	fn urls_follow_the_transport() {
		assert_eq!(url(DEFAULT_LISTEN, false, &[]), "http://127.0.0.1:7765/mcp");
		assert_eq!(url("[::1]:7765", true, &[]), "https://[::1]:7765/mcp");
		assert_eq!(
			url("100.64.0.1:8443", true, &["nas.example.ts.net".into()]),
			"https://nas.example.ts.net:8443/mcp"
		);
		assert_eq!(
			url("100.64.0.1:8443", false, &[]),
			"http://100.64.0.1:8443/mcp"
		);
	}
}
