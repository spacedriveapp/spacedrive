use serde::{Deserialize, Serialize};
use specta::Type;
use std::{collections::BTreeMap, path::PathBuf};

/// The process contract: what to run, where, and with which environment
/// changes. A key mapped to `None` is removed from the inherited environment;
/// `Some` values are set on top of it.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpawnSpec {
	pub cmd: Vec<String>,
	pub cwd: PathBuf,
	pub env: BTreeMap<String, Option<String>>,
}

/// How a service runs and how its liveness is judged.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ServiceKind {
	/// A long-running child process with an HTTP health endpoint. An
	/// already-healthy endpoint is adopted rather than respawned.
	Daemon {
		command: Vec<String>,
		cwd: PathBuf,
		#[serde(default)]
		env: BTreeMap<String, String>,
		health: String,
		/// Managed dev server: supervised and probed, but never respawned on
		/// exit and never converged to owned.
		#[serde(default)]
		dev: bool,
	},
	/// A docker compose stack addressed by its project directory. Never
	/// spawned as a child; lifecycle goes through the compose CLI.
	Compose {
		dir: PathBuf,
		health: String,
		/// Interpolation supplied to `docker compose` for commands that can
		/// (re)create containers.
		#[serde(default)]
		env: BTreeMap<String, String>,
	},
	/// Observed, not owned: probed and displayed, never spawned. The health
	/// URL may live on another machine.
	External {
		#[serde(default)]
		health: Option<String>,
	},
}

/// A named service under supervision.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ServiceDefinition {
	pub name: String,
	#[serde(flatten)]
	pub kind: ServiceKind,
}

impl ServiceKind {
	pub fn is_observed(&self) -> bool {
		matches!(self, Self::External { .. })
	}

	pub fn is_dev(&self) -> bool {
		matches!(self, Self::Daemon { dev: true, .. })
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum ProcessState {
	Starting,
	Running,
	Stopped,
	Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum Ownership {
	/// Spawned by this supervisor; exits are crashes to converge on.
	Owned,
	/// Found already running and left exactly as it was; death is only
	/// visible through failed probes.
	Adopted,
	/// Lifecycle delegated to the compose CLI.
	Compose,
	/// Observed on another controller's authority; never spawned or stopped.
	External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum HealthState {
	Ok,
	Failing,
	Unknown,
}

/// The observable state of one supervised service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ProcessStatus {
	pub name: String,
	pub state: ProcessState,
	pub ownership: Option<Ownership>,
	pub health: HealthState,
	pub pid: Option<u32>,
	/// ISO 8601 timestamp of the service's last transition into a running
	/// or starting state.
	pub since: Option<String>,
	pub restarts: u32,
	pub detail: String,
	pub dev: bool,
}
