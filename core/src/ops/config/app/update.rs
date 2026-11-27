//! Update app configuration action

use std::{path::PathBuf, sync::Arc};

use serde::{Deserialize, Serialize};
use specta::Type;
use tracing::info;

use crate::{
	config::AppConfig,
	context::CoreContext,
	infra::action::{error::ActionError, CoreAction},
};

/// Input for updating app configuration
/// All fields are optional for partial updates
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
pub struct UpdateAppConfigInput {
	/// Whether telemetry is enabled
	#[serde(skip_serializing_if = "Option::is_none")]
	pub telemetry_enabled: Option<bool>,

	/// Logging level
	#[serde(skip_serializing_if = "Option::is_none")]
	pub log_level: Option<String>,

	/// Theme preference (system, light, dark)
	#[serde(skip_serializing_if = "Option::is_none")]
	pub theme: Option<String>,

	/// Language preference (ISO 639-1 code)
	#[serde(skip_serializing_if = "Option::is_none")]
	pub language: Option<String>,

	/// Ceiling on the mount block cache, in bytes. Applies immediately —
	/// the cache evicts down to a lowered limit rather than waiting for a
	/// restart, because nothing in it is authoritative.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub mounts_cache_max_bytes: Option<u64>,

	/// Allow FUSE mounts on Linux. Mounts already up stay up when this is
	/// turned off; it only refuses new ones.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub mounts_fuse_enabled: Option<bool>,

	/// Allow localhost NFS mounts. Mounts already up stay up when this is
	/// turned off; it only refuses new ones.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub mounts_nfs_enabled: Option<bool>,

	/// Ceiling on replica bytes per second, serving and fetching combined;
	/// zero lifts it. Applies to transfers already in flight.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub replication_max_bytes_per_sec: Option<u64>,

	/// Displaced copies kept per library under `restore-trash/` after a
	/// successful `restore --replace`; the copy that restore just parked
	/// is kept whatever the number.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub backup_restore_trash_keep: Option<u32>,

	/// Whether the daemon serves the agent MCP endpoint. Applies at restart.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub agent_mcp_enabled: Option<bool>,

	/// The address the agent MCP endpoint listens on. Applies at restart.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub agent_mcp_listen: Option<String>,

	/// PEM certificate chain the agent MCP endpoint serves TLS with; an empty
	/// string clears it. Applies at restart.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub agent_mcp_tls_cert: Option<String>,

	/// PEM private key for the agent MCP certificate; an empty string clears
	/// it. Applies at restart.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub agent_mcp_tls_key: Option<String>,

	/// Let a non-loopback agent MCP listener serve plain HTTP. Applies at
	/// restart.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub agent_mcp_plaintext: Option<bool>,

	/// Host names a non-loopback agent MCP listener accepts besides its IP;
	/// an empty list clears them. Applies at restart.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub agent_mcp_host_names: Option<Vec<String>>,

	/// Whether networking is enabled
	#[serde(skip_serializing_if = "Option::is_none")]
	pub networking_enabled: Option<bool>,

	/// Whether volume monitoring is enabled
	#[serde(skip_serializing_if = "Option::is_none")]
	pub volume_monitoring_enabled: Option<bool>,

	/// Whether filesystem watcher is enabled
	#[serde(skip_serializing_if = "Option::is_none")]
	pub fs_watcher_enabled: Option<bool>,

	/// Whether statistics listener is enabled
	#[serde(skip_serializing_if = "Option::is_none")]
	pub statistics_listener_enabled: Option<bool>,

	/// Whether job logging is enabled
	#[serde(skip_serializing_if = "Option::is_none")]
	pub job_logging_enabled: Option<bool>,

	/// Whether to include debug logs in job logs
	#[serde(skip_serializing_if = "Option::is_none")]
	pub job_logging_include_debug: Option<bool>,

	/// Automatically accept vouches from trusted devices
	#[serde(skip_serializing_if = "Option::is_none")]
	pub proxy_pairing_auto_accept_vouched: Option<bool>,

	/// Automatically vouch new devices to all paired devices
	#[serde(skip_serializing_if = "Option::is_none")]
	pub proxy_pairing_auto_vouch_to_all: Option<bool>,

	/// Maximum age of vouch signatures in seconds
	#[serde(skip_serializing_if = "Option::is_none")]
	pub proxy_pairing_vouch_signature_max_age: Option<u64>,

	/// Timeout for proxy confirmation in seconds
	#[serde(skip_serializing_if = "Option::is_none")]
	pub proxy_pairing_vouch_response_timeout: Option<u64>,

	/// Maximum retries for queued vouches
	#[serde(skip_serializing_if = "Option::is_none")]
	pub proxy_pairing_vouch_queue_retry_limit: Option<u32>,
}

/// Output for update app configuration action
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct UpdateAppConfigOutput {
	/// Whether the update was successful
	pub success: bool,

	/// Message describing the result
	pub message: String,

	/// Whether a restart is recommended for changes to take effect
	pub requires_restart: bool,
}

/// Action to update app configuration
pub struct UpdateAppConfigAction {
	input: UpdateAppConfigInput,
}

impl CoreAction for UpdateAppConfigAction {
	type Input = UpdateAppConfigInput;
	type Output = UpdateAppConfigOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		if let Some(level) = &input.log_level {
			let valid_levels = ["trace", "debug", "info", "warn", "error"];
			if !valid_levels.contains(&level.to_lowercase().as_str()) {
				return Err(format!(
					"Invalid log level '{}'. Must be one of: {}",
					level,
					valid_levels.join(", ")
				));
			}
		}

		if let Some(theme) = &input.theme {
			let valid_themes = [
				"system", "light", "dark", "midnight", "noir", "slate", "nord", "mocha",
			];
			if !valid_themes.contains(&theme.to_lowercase().as_str()) {
				return Err(format!(
					"Invalid theme '{}'. Must be one of: {}",
					theme,
					valid_themes.join(", ")
				));
			}
		}

		// A basic ISO 639-1 check
		if let Some(lang) = &input.language {
			if lang.len() != 2 || !lang.chars().all(|c| c.is_ascii_lowercase()) {
				return Err(
					"Language must be a 2-letter ISO 639-1 code (e.g., 'en', 'de')".to_string(),
				);
			}
		}

		if let Some(listen) = &input.agent_mcp_listen {
			crate::service::agent_mcp::listen_addr(listen)?;
		}
		if let Some(names) = &input.agent_mcp_host_names {
			for name in names {
				crate::service::agent_mcp::host_name(name)?;
			}
		}

		if input.proxy_pairing_vouch_signature_max_age == Some(0) {
			return Err("Signature max age must be greater than 0".to_string());
		}
		if input.proxy_pairing_vouch_response_timeout == Some(0) {
			return Err("Response timeout must be greater than 0".to_string());
		}
		if input.proxy_pairing_vouch_queue_retry_limit == Some(0) {
			return Err("Retry limit must be greater than 0".to_string());
		}

		Ok(Self { input })
	}

	async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output, ActionError> {
		let mut config = AppConfig::load_from(&context.data_dir)
			.map_err(|e| ActionError::Internal(format!("Failed to load config: {}", e)))?;

		let mut requires_restart = false;
		let mut changes = Vec::new();

		// Apply updates
		if let Some(telemetry_enabled) = self.input.telemetry_enabled {
			if config.telemetry_enabled != telemetry_enabled {
				config.telemetry_enabled = telemetry_enabled;
				changes.push("telemetry_enabled");
			}
		}

		if let Some(ref log_level) = self.input.log_level {
			if config.log_level != *log_level {
				config.log_level = log_level.to_lowercase();
				changes.push("log_level");
				requires_restart = true;
			}
		}

		if let Some(ref theme) = self.input.theme {
			if config.preferences.theme != *theme {
				config.preferences.theme = theme.to_lowercase();
				changes.push("theme");
			}
		}

		if let Some(ref language) = self.input.language {
			if config.preferences.language != *language {
				config.preferences.language = language.clone();
				changes.push("language");
			}
		}

		if let Some(cache_max_bytes) = self.input.mounts_cache_max_bytes {
			if config.mounts.cache_max_bytes != cache_max_bytes {
				config.mounts.cache_max_bytes = cache_max_bytes;
				changes.push("mounts_cache_max_bytes");
				if let Some(cache) = crate::service::mounts::cache::cache() {
					cache.set_max_bytes(cache_max_bytes).await;
				}
			}
		}

		if let Some(fuse_enabled) = self.input.mounts_fuse_enabled {
			if config.mounts.fuse_enabled != fuse_enabled {
				config.mounts.fuse_enabled = fuse_enabled;
				changes.push("mounts_fuse_enabled");
			}
		}

		if let Some(nfs_enabled) = self.input.mounts_nfs_enabled {
			if config.mounts.nfs_enabled != nfs_enabled {
				config.mounts.nfs_enabled = nfs_enabled;
				changes.push("mounts_nfs_enabled");
			}
		}

		if let Some(max_bytes_per_sec) = self.input.replication_max_bytes_per_sec {
			if config.replication.max_bytes_per_sec != max_bytes_per_sec {
				config.replication.max_bytes_per_sec = max_bytes_per_sec;
				changes.push("replication_max_bytes_per_sec");
				crate::service::mounts::replication::configure(&config.replication);
			}
		}

		if let Some(keep) = self.input.backup_restore_trash_keep {
			if config.backup.restore_trash_keep != keep {
				config.backup.restore_trash_keep = keep;
				changes.push("backup_restore_trash_keep");
			}
		}

		if let Some(enabled) = self.input.agent_mcp_enabled {
			if config.agent_mcp.enabled != enabled {
				config.agent_mcp.enabled = enabled;
				changes.push("agent_mcp_enabled");
				requires_restart = true;
			}
		}

		if let Some(ref listen) = self.input.agent_mcp_listen {
			if config.agent_mcp.listen != *listen {
				config.agent_mcp.listen = listen.clone();
				changes.push("agent_mcp_listen");
				requires_restart = true;
			}
		}

		if let Some(ref cert) = self.input.agent_mcp_tls_cert {
			let cert = Some(cert.trim())
				.filter(|c| !c.is_empty())
				.map(PathBuf::from);
			if config.agent_mcp.tls_cert != cert {
				config.agent_mcp.tls_cert = cert;
				changes.push("agent_mcp_tls_cert");
				requires_restart = true;
			}
		}

		if let Some(ref key) = self.input.agent_mcp_tls_key {
			let key = Some(key.trim())
				.filter(|k| !k.is_empty())
				.map(PathBuf::from);
			if config.agent_mcp.tls_key != key {
				config.agent_mcp.tls_key = key;
				changes.push("agent_mcp_tls_key");
				requires_restart = true;
			}
		}

		if let Some(plaintext) = self.input.agent_mcp_plaintext {
			if config.agent_mcp.plaintext != plaintext {
				config.agent_mcp.plaintext = plaintext;
				changes.push("agent_mcp_plaintext");
				requires_restart = true;
			}
		}

		if let Some(ref names) = self.input.agent_mcp_host_names {
			if config.agent_mcp.host_names != *names {
				config.agent_mcp.host_names = names.clone();
				changes.push("agent_mcp_host_names");
				requires_restart = true;
			}
		}

		if let Some(networking_enabled) = self.input.networking_enabled {
			if config.services.networking_enabled != networking_enabled {
				config.services.networking_enabled = networking_enabled;
				changes.push("networking_enabled");
				requires_restart = true;
			}
		}

		if let Some(volume_monitoring_enabled) = self.input.volume_monitoring_enabled {
			if config.services.volume_monitoring_enabled != volume_monitoring_enabled {
				config.services.volume_monitoring_enabled = volume_monitoring_enabled;
				changes.push("volume_monitoring_enabled");
				requires_restart = true;
			}
		}

		if let Some(fs_watcher_enabled) = self.input.fs_watcher_enabled {
			if config.services.fs_watcher_enabled != fs_watcher_enabled {
				config.services.fs_watcher_enabled = fs_watcher_enabled;
				changes.push("fs_watcher_enabled");
				requires_restart = true;
			}
		}

		if let Some(statistics_listener_enabled) = self.input.statistics_listener_enabled {
			if config.services.statistics_listener_enabled != statistics_listener_enabled {
				config.services.statistics_listener_enabled = statistics_listener_enabled;
				changes.push("statistics_listener_enabled");
				requires_restart = true;
			}
		}

		if let Some(job_logging_enabled) = self.input.job_logging_enabled {
			if config.job_logging.enabled != job_logging_enabled {
				config.job_logging.enabled = job_logging_enabled;
				changes.push("job_logging_enabled");
			}
		}

		if let Some(job_logging_include_debug) = self.input.job_logging_include_debug {
			if config.job_logging.include_debug != job_logging_include_debug {
				config.job_logging.include_debug = job_logging_include_debug;
				changes.push("job_logging_include_debug");
			}
		}

		if let Some(auto_accept_vouched) = self.input.proxy_pairing_auto_accept_vouched {
			if config.proxy_pairing.auto_accept_vouched != auto_accept_vouched {
				config.proxy_pairing.auto_accept_vouched = auto_accept_vouched;
				changes.push("proxy_pairing_auto_accept_vouched");
			}
		}

		if let Some(auto_vouch_to_all) = self.input.proxy_pairing_auto_vouch_to_all {
			if config.proxy_pairing.auto_vouch_to_all != auto_vouch_to_all {
				config.proxy_pairing.auto_vouch_to_all = auto_vouch_to_all;
				changes.push("proxy_pairing_auto_vouch_to_all");
			}
		}

		if let Some(max_age) = self.input.proxy_pairing_vouch_signature_max_age {
			if config.proxy_pairing.vouch_signature_max_age != max_age {
				config.proxy_pairing.vouch_signature_max_age = max_age;
				changes.push("proxy_pairing_vouch_signature_max_age");
			}
		}

		if let Some(timeout) = self.input.proxy_pairing_vouch_response_timeout {
			if config.proxy_pairing.vouch_response_timeout != timeout {
				config.proxy_pairing.vouch_response_timeout = timeout;
				changes.push("proxy_pairing_vouch_response_timeout");
			}
		}

		if let Some(retry_limit) = self.input.proxy_pairing_vouch_queue_retry_limit {
			if config.proxy_pairing.vouch_queue_retry_limit != retry_limit {
				config.proxy_pairing.vouch_queue_retry_limit = retry_limit;
				changes.push("proxy_pairing_vouch_queue_retry_limit");
			}
		}

		if changes.is_empty() {
			return Ok(UpdateAppConfigOutput {
				success: true,
				message: "No changes to apply".to_string(),
				requires_restart: false,
			});
		}

		config
			.save()
			.map_err(|e| ActionError::Internal(format!("Failed to save config: {}", e)))?;

		if let Some(networking) = context.get_networking().await {
			let registry = networking.protocol_registry();
			let guard = registry.read().await;
			if let Some(handler) = guard.get_handler("pairing") {
				if let Some(pairing) = handler
					.as_any()
					.downcast_ref::<crate::service::network::protocol::PairingProtocolHandler>(
				) {
					pairing.set_proxy_config(config.proxy_pairing.clone()).await;
				}
			}
		}

		// Emit config change events for each changed field
		for field in &changes {
			context
				.events
				.emit(crate::infra::event::Event::ConfigChanged {
					field: field.to_string(),
				});
		}

		info!(
			changes = ?changes,
			requires_restart = requires_restart,
			"App configuration updated"
		);

		Ok(UpdateAppConfigOutput {
			success: true,
			message: format!("Updated: {}", changes.join(", ")),
			requires_restart,
		})
	}

	fn action_kind(&self) -> &'static str {
		"config.app.update"
	}
}

crate::register_core_action!(UpdateAppConfigAction, "config.app.update");
