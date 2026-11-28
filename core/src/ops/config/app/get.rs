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

	/// Mounted-source behaviour
	pub mounts: MountsConfigOutput,

	/// Replica transfers with paired devices
	pub replication: ReplicationConfigOutput,

	/// Library backup and restore
	pub backup: BackupConfigOutput,

	/// The agent MCP endpoint
	pub agent_mcp: AgentMcpConfigOutput,

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

/// Mounts configuration output
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MountsConfigOutput {
	pub cache_max_bytes: u64,
	/// Whether FUSE mounts are allowed on this device.
	pub fuse_enabled: bool,
}

/// Replication configuration output
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ReplicationConfigOutput {
	/// Bytes per second across every replica transfer; zero is unlimited.
	pub max_bytes_per_sec: u64,
	/// Whether this device has stopped fetching replicas.
	pub paused: bool,
}

/// Backup configuration output
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct BackupConfigOutput {
	/// Displaced copies kept per library under `restore-trash/` after a
	/// successful `restore --replace`.
	pub restore_trash_keep: u32,
}

/// Agent MCP endpoint configuration output
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct AgentMcpConfigOutput {
	/// Whether the daemon serves the endpoint.
	pub enabled: bool,
	/// The address it listens on when enabled.
	pub listen: String,
	/// PEM certificate chain served when TLS is on.
	pub tls_cert: Option<PathBuf>,
	/// PEM private key for `tls_cert`.
	pub tls_key: Option<PathBuf>,
	/// Whether a non-loopback listener may serve plain HTTP.
	pub plaintext: bool,
	/// Names a non-loopback listener accepts in Host besides its IP.
	pub host_names: Vec<String>,
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
			mounts: MountsConfigOutput {
				cache_max_bytes: config.mounts.cache_max_bytes,
				fuse_enabled: config.mounts.fuse_enabled,
			},
			replication: ReplicationConfigOutput {
				max_bytes_per_sec: config.replication.max_bytes_per_sec,
				paused: config.replication.paused,
			},
			backup: BackupConfigOutput {
				restore_trash_keep: config.backup.restore_trash_keep,
			},
			agent_mcp: AgentMcpConfigOutput {
				enabled: config.agent_mcp.enabled,
				listen: config.agent_mcp.listen.clone(),
				tls_cert: config.agent_mcp.tls_cert.clone(),
				tls_key: config.agent_mcp.tls_key.clone(),
				plaintext: config.agent_mcp.plaintext,
				host_names: config.agent_mcp.host_names.clone(),
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
