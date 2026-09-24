//! Update app configuration action

use std::sync::Arc;

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
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
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

	/// Whether Spacebot features are enabled in the UI
	#[serde(skip_serializing_if = "Option::is_none")]
	pub spacebot_enabled: Option<bool>,

	/// Spacebot API base URL
	#[serde(skip_serializing_if = "Option::is_none")]
	pub spacebot_base_url: Option<String>,

	/// Optional Spacebot bearer token
	#[serde(skip_serializing_if = "Option::is_none")]
	pub spacebot_auth_token: Option<String>,

	/// Default Spacebot agent ID for embedded chat
	#[serde(skip_serializing_if = "Option::is_none")]
	pub spacebot_default_agent_id: Option<String>,

	/// Default sender name for embedded chat
	#[serde(skip_serializing_if = "Option::is_none")]
	pub spacebot_default_sender_name: Option<String>,
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

		if input.proxy_pairing_vouch_signature_max_age == Some(0) {
			return Err("Signature max age must be greater than 0".to_string());
		}
		if input.proxy_pairing_vouch_response_timeout == Some(0) {
			return Err("Response timeout must be greater than 0".to_string());
		}
		if input.proxy_pairing_vouch_queue_retry_limit == Some(0) {
			return Err("Retry limit must be greater than 0".to_string());
		}

		for (field, value) in [
			("Spacebot base URL", &input.spacebot_base_url),
			("Default agent ID", &input.spacebot_default_agent_id),
			("Default sender name", &input.spacebot_default_sender_name),
		] {
			if value.as_ref().is_some_and(|value| value.trim().is_empty()) {
				return Err(format!("{field} cannot be empty"));
			}
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

		if let Some(spacebot_enabled) = self.input.spacebot_enabled {
			if config.spacebot.enabled != spacebot_enabled {
				config.spacebot.enabled = spacebot_enabled;
				changes.push("spacebot_enabled");
			}
		}

		if let Some(ref spacebot_base_url) = self.input.spacebot_base_url {
			let normalized = spacebot_base_url.trim().trim_end_matches('/').to_string();
			if config.spacebot.base_url != normalized {
				config.spacebot.base_url = normalized;
				changes.push("spacebot_base_url");
			}
		}

		if let Some(ref spacebot_auth_token) = self.input.spacebot_auth_token {
			let normalized = if spacebot_auth_token.trim().is_empty() {
				None
			} else {
				Some(spacebot_auth_token.trim().to_string())
			};
			if config.spacebot.auth_token != normalized {
				config.spacebot.auth_token = normalized;
				changes.push("spacebot_auth_token");
			}
		}

		if let Some(ref spacebot_default_agent_id) = self.input.spacebot_default_agent_id {
			let normalized = spacebot_default_agent_id.trim().to_string();
			if config.spacebot.default_agent_id != normalized {
				config.spacebot.default_agent_id = normalized;
				changes.push("spacebot_default_agent_id");
			}
		}

		if let Some(ref spacebot_default_sender_name) = self.input.spacebot_default_sender_name {
			let normalized = spacebot_default_sender_name.trim().to_string();
			if config.spacebot.default_sender_name != normalized {
				config.spacebot.default_sender_name = normalized;
				changes.push("spacebot_default_sender_name");
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
