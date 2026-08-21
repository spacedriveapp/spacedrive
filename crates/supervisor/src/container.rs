use crate::exec::{exec, which};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{collections::BTreeMap, path::Path};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum ContainerProvider {
	Docker,
	Colima,
}

/// Which container runtime compose services talk to.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ContainerRuntime {
	pub provider: ContainerProvider,
	pub profile: String,
}

impl Default for ContainerRuntime {
	fn default() -> Self {
		Self {
			provider: ContainerProvider::Docker,
			profile: "default".to_string(),
		}
	}
}

#[derive(Debug, Clone)]
pub struct RuntimeStatus {
	/// The daemon answered, so a container can actually run here.
	pub reachable: bool,
	/// Server version when reachable, the refusal reason when not.
	pub detail: String,
}

// Colima registers one docker context per profile: `colima` for the default
// profile and `colima-<profile>` for any other. Addressing it through
// DOCKER_CONTEXT keeps the operator's own `docker context use` selection
// intact — a supervisor choosing a runtime must not move their shell's.
fn context_name(runtime: &ContainerRuntime) -> Option<String> {
	if runtime.provider != ContainerProvider::Colima {
		return None;
	}
	Some(if runtime.profile == "default" {
		"colima".to_string()
	} else {
		format!("colima-{}", runtime.profile)
	})
}

/// Environment that points a docker invocation at the runtime. Every command
/// that reaches the daemon has to carry it.
pub fn container_env(runtime: &ContainerRuntime) -> BTreeMap<String, String> {
	let mut env = BTreeMap::new();
	if let Some(context) = context_name(runtime) {
		env.insert("DOCKER_CONTEXT".to_string(), context);
	}
	env
}

// `docker compose version` is answered by the client alone: it prints a
// version on a machine whose daemon is stopped, so it witnesses nothing about
// whether a container can run. `docker version` asks the server and exits
// non-zero when it cannot be reached.
pub async fn probe_container_runtime(runtime: &ContainerRuntime) -> RuntimeStatus {
	let cmd = ["docker", "version", "--format", "{{.Server.Version}}"]
		.map(String::from)
		.to_vec();
	match exec(&cmd, Path::new("/"), &container_env(runtime)).await {
		Ok(result) if result.code == 0 => RuntimeStatus {
			reachable: true,
			detail: format!("docker server {}", result.stdout.trim()),
		},
		Ok(result) => {
			let reason = result.reason();
			RuntimeStatus {
				reachable: false,
				detail: if reason.is_empty() {
					"docker daemon unreachable".to_string()
				} else {
					reason
				},
			}
		}
		Err(err) => RuntimeStatus {
			reachable: false,
			detail: err.to_string(),
		},
	}
}

/// Adopt-first, the same rule every service follows: a reachable runtime is
/// used as found and never restarted. Only a colima profile the supervisor was
/// told to manage is started, and it is never stopped — the machine's
/// container runtime is not taken down on the way out.
pub async fn ensure_container_runtime(
	runtime: &ContainerRuntime,
) -> Result<RuntimeStatus, String> {
	let probe = probe_container_runtime(runtime).await;
	if probe.reachable || runtime.provider != ContainerProvider::Colima {
		return Ok(probe);
	}
	if !which("colima") {
		return Err(
			"colima not found on PATH — install it or set the container provider to \"docker\""
				.to_string(),
		);
	}
	let cmd = ["colima", "start", "--profile", runtime.profile.as_str()]
		.map(String::from)
		.to_vec();
	let start = exec(&cmd, Path::new("/"), &BTreeMap::new())
		.await
		.map_err(|e| e.to_string())?;
	if start.code != 0 {
		return Err(format!(
			"colima start --profile {} failed: {}",
			runtime.profile,
			start.reason()
		));
	}
	Ok(probe_container_runtime(runtime).await)
}
