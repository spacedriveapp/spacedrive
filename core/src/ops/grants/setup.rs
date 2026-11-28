//! What an agent needs to connect with a grant from this daemon: the MCP
//! endpoint's URL, whether it is on, and the skill `get_skill` returns.
//!
//! The app mints a grant and then shows setup for it. It reads the URL here
//! rather than from the raw `agent_mcp.listen` setting so it shows the
//! address the endpoint actually binds, as the CLI's `--print-mcp-config`
//! does.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
	config::{AgentMcpConfig, AppConfig},
	context::CoreContext,
	infra::query::{CoreQuery, QueryError, QueryResult},
	service::agent_mcp::{url, SKILL},
};

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
pub struct GrantSetupInput {}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GrantSetupOutput {
	/// Whether the daemon is configured to serve the agent MCP endpoint.
	/// Turning it on takes effect after a restart.
	pub endpoint_enabled: bool,
	/// The URL MCP clients connect to, such as `http://127.0.0.1:7765/mcp`.
	pub mcp_url: String,
	/// The guide the endpoint's `get_skill` tool returns, in Markdown.
	pub skill: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrantSetupQuery;

impl CoreQuery for GrantSetupQuery {
	type Input = GrantSetupInput;
	type Output = GrantSetupOutput;

	fn from_input(_input: Self::Input) -> QueryResult<Self> {
		Ok(Self)
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		_session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let data_dir = context.data_dir.clone();
		let config = tokio::task::spawn_blocking(move || AppConfig::load_from(&data_dir))
			.await
			.map_err(|e| QueryError::Internal(e.to_string()))?
			.map_err(|e| QueryError::Internal(format!("load config: {e}")))?;
		Ok(GrantSetupOutput {
			endpoint_enabled: config.agent_mcp.enabled,
			mcp_url: mcp_url(&config.agent_mcp),
			skill: SKILL.to_string(),
		})
	}
}

/// The URL for the endpoint `config` describes: https when a certificate and
/// key are set. A listen setting the endpoint would refuse falls back to the
/// default address, which is where it binds once the setting is fixed.
pub fn mcp_url(config: &AgentMcpConfig) -> String {
	let tls = config.tls_cert.is_some() && config.tls_key.is_some();
	url(&config.listen, tls, &config.host_names)
}

crate::register_core_query!(GrantSetupQuery, "grants.setup");

#[cfg(test)]
mod tests {
	use super::{mcp_url, AgentMcpConfig};

	fn listening(listen: &str) -> AgentMcpConfig {
		AgentMcpConfig {
			listen: listen.to_string(),
			..AgentMcpConfig::default()
		}
	}

	#[test]
	fn url_names_the_bound_address() {
		assert_eq!(
			mcp_url(&listening("127.0.0.1:7765")),
			"http://127.0.0.1:7765/mcp"
		);
		assert_eq!(
			mcp_url(&listening("localhost:9000")),
			"http://127.0.0.1:9000/mcp"
		);
		assert_eq!(
			mcp_url(&listening("0.0.0.0:7765")),
			"http://127.0.0.1:7765/mcp"
		);
	}

	#[test]
	fn url_uses_https_when_tls_is_configured() {
		let config = AgentMcpConfig {
			tls_cert: Some("cert.pem".into()),
			tls_key: Some("key.pem".into()),
			..listening("127.0.0.1:7765")
		};
		assert_eq!(mcp_url(&config), "https://127.0.0.1:7765/mcp");
	}
}
