//! The host's process manager: one supervisor per machine, its persisted
//! service definitions, and the bridge from supervisor state changes to
//! resource events.

use super::resource::Process;
use crate::{domain::resource::EventEmitter, infra::event::EventBus};
use sd_supervisor::{
	ContainerRuntime, ServiceDefinition, Supervisor, SupervisorConfig, SupervisorError, Timing,
};
use std::{
	io,
	path::{Path, PathBuf},
	sync::Arc,
};
use tracing::{error, warn};

/// Where the supervisor keeps its state, relative to the daemon data dir.
const PROCESSES_DIR: &str = "processes";
const DEFINITIONS_FILE: &str = "services.json";
const LEDGER_FILE: &str = "ports.json";
const LOGS_DIR: &str = "logs";

pub struct ProcessManager {
	supervisor: Supervisor,
	definitions_path: PathBuf,
}

impl ProcessManager {
	/// Build the manager, loading any persisted service definitions. Failures
	/// to read persisted state degrade to an empty registry rather than
	/// failing daemon startup.
	pub async fn new(data_dir: &Path, events: Arc<EventBus>) -> Arc<Self> {
		let dir = data_dir.join(PROCESSES_DIR);
		let supervisor = Supervisor::new(SupervisorConfig {
			logs_dir: dir.join(LOGS_DIR),
			ledger_path: dir.join(LEDGER_FILE),
			timing: Timing::default(),
			containers: ContainerRuntime::default(),
		});

		let manager = Arc::new(Self {
			supervisor,
			definitions_path: dir.join(DEFINITIONS_FILE),
		});

		for definition in manager.load_definitions() {
			let name = definition.name.clone();
			if let Err(err) = manager.supervisor.register(definition).await {
				warn!("skipping persisted service {name}: {err}");
			}
		}

		// Forward every supervisor state change onto the event bus as a
		// resource update, so shells and agents observe one process table.
		let mut changes = manager.supervisor.subscribe();
		tokio::spawn(async move {
			while let Ok(status) = changes.recv().await {
				let process = Process::from(status);
				if let Err(err) = process.emit_changed(&events) {
					error!("failed to emit process event: {err}");
				}
			}
		});

		manager
	}

	pub fn supervisor(&self) -> &Supervisor {
		&self.supervisor
	}

	/// Register a service and persist its definition so the daemon
	/// re-supervises it on the next boot.
	pub async fn register(&self, definition: ServiceDefinition) -> Result<(), SupervisorError> {
		self.supervisor.register(definition).await?;
		if let Err(err) = self.persist_definitions().await {
			warn!("failed to persist service definitions: {err}");
		}
		Ok(())
	}

	fn load_definitions(&self) -> Vec<ServiceDefinition> {
		match std::fs::read_to_string(&self.definitions_path) {
			Ok(raw) => serde_json::from_str(&raw).unwrap_or_else(|err| {
				warn!(
					"unreadable service definitions at {}: {err}",
					self.definitions_path.display()
				);
				Vec::new()
			}),
			Err(_) => Vec::new(),
		}
	}

	async fn persist_definitions(&self) -> io::Result<()> {
		let definitions = self.supervisor.definitions().await;
		if let Some(parent) = self.definitions_path.parent() {
			std::fs::create_dir_all(parent)?;
		}
		let mut body = serde_json::to_string_pretty(&definitions).map_err(io::Error::other)?;
		body.push('\n');
		let temporary = self.definitions_path.with_extension("json.tmp");
		std::fs::write(&temporary, body)?;
		std::fs::rename(&temporary, &self.definitions_path)
	}
}
