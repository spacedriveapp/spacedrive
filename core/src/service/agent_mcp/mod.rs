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
//! It is off by default and binds loopback only. Listening on any other
//! address is refused until the checklist in
//! `docs/core/design/agent-access-threat-model.md` is met, since that needs
//! TLS and handle-based path containment this build does not have.
//!
//! ## Example
//! ```rust,no_run
//! # async fn demo(context: std::sync::Arc<sd_core::context::CoreContext>) -> anyhow::Result<()> {
//! let bound = sd_core::service::agent_mcp::start(context, "127.0.0.1:7765").await?;
//! assert!(bound.ip().is_loopback());
//! # Ok(())
//! # }
//! ```

mod http;
mod limits;
mod oauth;
mod paths;
mod tools;

use std::{
	net::{IpAddr, Ipv4Addr, SocketAddr},
	sync::Arc,
};

use tokio::net::TcpListener;
use tracing::{error, info};

use crate::context::CoreContext;

pub use limits::Limits;

/// The port the endpoint listens on unless the config names another.
pub const DEFAULT_PORT: u16 = 7765;
/// The default value of `agent_mcp.listen`.
pub const DEFAULT_LISTEN: &str = "127.0.0.1:7765";
/// The URL path MCP clients connect to.
pub const MCP_PATH: &str = "/mcp";
/// The skill `get_skill` returns. It is compiled in, never read from a grant.
pub const SKILL: &str = include_str!("SKILL.md");

const NON_LOOPBACK: &str = "the agent MCP endpoint listens on loopback only. Listening on another address needs TLS and handle-based path containment (docs/core/design/agent-access-threat-model.md), which this build does not have";

/// The socket address `listen` names, or why the endpoint will not bind it.
///
/// Accepts `127.0.0.1:7765`, `[::1]:7765`, `localhost:7765`, or an address
/// without a port, which gets [`DEFAULT_PORT`]. Wildcards and every
/// non-loopback address are refused.
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
			"'{listen}' is not an address such as {DEFAULT_LISTEN}"
		));
	};
	if addr.ip().is_unspecified() {
		return Err(format!(
			"'{listen}' is a wildcard address; the agent MCP endpoint binds one loopback address"
		));
	}
	if !addr.ip().is_loopback() {
		return Err(format!("refusing to listen on {addr}: {NON_LOOPBACK}"));
	}
	Ok(addr)
}

/// Bind the endpoint on `listen` and serve it for the life of the process.
///
/// A refused address is logged with the reason and returned as an error, so
/// the daemon keeps running without the endpoint.
pub async fn start(context: Arc<CoreContext>, listen: &str) -> anyhow::Result<SocketAddr> {
	let addr = listen_addr(listen).map_err(|reason| {
		error!("Agent MCP endpoint not started: {reason}");
		anyhow::anyhow!(reason)
	})?;
	let listener = TcpListener::bind(addr).await?;
	let bound = serve(context, listener, Limits::default())?;
	info!("Agent MCP endpoint at http://{bound}{MCP_PATH}");
	Ok(bound)
}

/// Serve the endpoint on a listener that is already bound to loopback.
///
/// Tests bind an ephemeral port and pass lower limits through `limits`.
pub fn serve(
	context: Arc<CoreContext>,
	listener: TcpListener,
	limits: Limits,
) -> anyhow::Result<SocketAddr> {
	let bound = listener.local_addr()?;
	if !bound.ip().is_loopback() {
		anyhow::bail!("refusing to serve on {bound}: {NON_LOOPBACK}");
	}
	let endpoint = Arc::new(http::Endpoint::new(context, bound, limits));
	let app = http::router(endpoint).into_make_service_with_connect_info::<SocketAddr>();
	tokio::spawn(async move {
		if let Err(err) = axum::serve(listener, app).await {
			error!("Agent MCP endpoint exited: {err}");
		}
	});
	Ok(bound)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn loopback_addresses_are_accepted() {
		assert_eq!(listen_addr(DEFAULT_LISTEN).unwrap().port(), DEFAULT_PORT);
		assert_eq!(listen_addr("127.0.0.1").unwrap().port(), DEFAULT_PORT);
		assert_eq!(listen_addr("[::1]:9000").unwrap().port(), 9000);
		assert_eq!(
			listen_addr("localhost:9001").unwrap(),
			"127.0.0.1:9001".parse().unwrap()
		);
	}

	#[test]
	fn wildcard_and_remote_addresses_are_refused() {
		for listen in [
			"0.0.0.0:7765",
			"[::]:7765",
			"100.64.0.1:7765",
			"192.168.1.5",
			"example.com:7765",
			"localhost:port",
		] {
			assert!(listen_addr(listen).is_err(), "{listen} should be refused");
		}
	}
}
