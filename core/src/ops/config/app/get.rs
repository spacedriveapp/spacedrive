//! Get app configuration query

use std::{path::PathBuf, sync::Arc};

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
	config::{AppConfig, JobLoggingConfig, LoggingConfig, Preferences, ServiceConfig},
	context::CoreContext,
	infra::query::{CoreQuery, QueryError, QueryResult},
};

/// Input for getting app configuration
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetAppConfigQueryInput;

/// App configuration response with all fields exposed
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct AppConfigOutput {
	/// Config schema version
	pub version: u32,

	/// Data directory path
	pub data_dir: PathBuf,

	/// Logging level
	pub log_level: String,

	/// Whether telemetry is enabled
	pub telemetry_enabled: bool,

	/// User preferences
	pub preferences: PreferencesOutput,

	/// Job logging configuration
	pub job_logging: JobLoggingConfigOutput,

	/// Service configuration
	pub services: ServiceConfigOutput,

	/// Daemon logging configuration
	pub logging: LoggingConfigOutput,

	/// Proxy pairing configuration
	pub proxy_pairing: ProxyPairingConfigOutput,
}

/// User preferences output
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PreferencesOutput {
	pub theme: String,
	pub language: String,
}

/// Job logging configuration output
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct JobLoggingConfigOutput {
	pub enabled: bool,
	pub log_directory: String,
	pub max_file_size: u64,
	pub include_debug: bool,
	pub log_ephemeral_jobs: bool,
}

/// Service configuration output
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ServiceConfigOutput {
	pub networking_enabled: bool,
	pub volume_monitoring_enabled: bool,
	pub fs_watcher_enabled: bool,
	pub statistics_listener_enabled: bool,
}

/// Logging configuration output
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LoggingConfigOutput {
	pub main_filter: String,
}

/// Proxy pairing configuration output
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ProxyPairingConfigOutput {
	pub auto_accept_vouched: bool,
	pub auto_vouch_to_all: bool,
	pub vouch_signature_max_age: u64,
	pub vouch_response_timeout: u64,
	pub vouch_queue_retry_limit: u32,
}

impl From<&AppConfig> for AppConfigOutput {
	fn from(config: &AppConfig) -> Self {
		Self {
			version: config.version,
			data_dir: config.data_dir.clone(),
			log_level: config.log_level.clone(),
			telemetry_enabled: config.telemetry_enabled,
			preferences: PreferencesOutput {
				theme: config.preferences.theme.clone(),
				language: config.preferences.language.clone(),
			},
			job_logging: JobLoggingConfigOutput {
				enabled: config.job_logging.enabled,
				log_directory: config.job_logging.log_directory.clone(),
				max_file_size: config.job_logging.max_file_size,
				include_debug: config.job_logging.include_debug,
				log_ephemeral_jobs: config.job_logging.log_ephemeral_jobs,
			},
			services: ServiceConfigOutput {
				networking_enabled: config.services.networking_enabled,
				volume_monitoring_enabled: config.services.volume_monitoring_enabled,
				fs_watcher_enabled: config.services.fs_watcher_enabled,
				statistics_listener_enabled: config.services.statistics_listener_enabled,
			},
			logging: LoggingConfigOutput {
				main_filter: config.logging.main_filter.clone(),
			},
			proxy_pairing: ProxyPairingConfigOutput {
				auto_accept_vouched: config.proxy_pairing.auto_accept_vouched,
				auto_vouch_to_all: config.proxy_pairing.auto_vouch_to_all,
				vouch_signature_max_age: config.proxy_pairing.vouch_signature_max_age,
				vouch_response_timeout: config.proxy_pairing.vouch_response_timeout,
				vouch_queue_retry_limit: config.proxy_pairing.vouch_queue_retry_limit,
			},
		}
	}
}

/// Query to get app configuration
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetAppConfigQuery;

impl CoreQuery for GetAppConfigQuery {
	type Input = GetAppConfigQueryInput;
	type Output = AppConfigOutput;

	fn from_input(_input: Self::Input) -> QueryResult<Self> {
		Ok(Self)
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		_session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let config = AppConfig::load_from(&context.data_dir)
			.map_err(|e| QueryError::Internal(format!("Failed to load config: {}", e)))?;

		Ok(AppConfigOutput::from(&config))
	}
}

crate::register_core_query!(GetAppConfigQuery, "config.app.get");
