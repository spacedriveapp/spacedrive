//! Daemon lifecycle helpers: liveness checks and background spawning.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{Context, Result};
use sd_core::infra::daemon::types::{DaemonRequest, DaemonResponse};

use crate::client::CoreClient;

/// The daemon executable's file name next to the CLI: `sd-daemon`, or
/// `sd-daemon.exe` on Windows.
pub fn daemon_binary_name() -> String {
	format!("sd-daemon{}", std::env::consts::EXE_SUFFIX)
}

/// How to launch the daemon executable when it is not already running.
pub struct DaemonLaunchConfig {
	/// Path to the daemon executable.
	pub daemon_path: PathBuf,
	/// Base data directory passed via `--data-dir`. The daemon resolves
	/// `instance` under it itself, so this is never the instance's own
	/// directory; passing that nests the instance a second time.
	pub base_data_dir: PathBuf,
	/// Instance name passed via `--instance`, if any.
	pub instance: Option<String>,
	/// Passed as `--no-default-sources`: index only the sources added to the
	/// daemon's libraries.
	pub no_default_sources: bool,
}

impl DaemonLaunchConfig {
	/// Build the launch command with arguments and working directory set,
	/// leaving stdio configuration to the caller.
	pub fn command(&self) -> std::io::Result<Command> {
		let mut command = Command::new(&self.daemon_path);
		command.arg("--data-dir").arg(&self.base_data_dir);
		if let Some(instance) = &self.instance {
			command.arg("--instance").arg(instance);
		}
		if self.no_default_sources {
			command.arg("--no-default-sources");
		}
		command.current_dir(std::env::current_dir()?);
		Ok(command)
	}
}

/// Result of [`ensure_daemon`].
pub enum EnsureDaemonOutcome {
	/// The daemon was already answering pings; nothing was spawned.
	AlreadyRunning,
	/// A daemon process was spawned in the background.
	Started {
		/// PID of the spawned process.
		pid: u32,
		/// Whether the daemon answered a ping after the startup grace period.
		responsive: bool,
	},
}

/// Check whether a daemon is answering pings at the client's socket address.
pub async fn is_daemon_running(client: &CoreClient) -> bool {
	matches!(
		client.send_raw_request(&DaemonRequest::Ping).await,
		Ok(DaemonResponse::Pong)
	)
}

/// Ping the daemon and spawn it in the background if it is not running.
///
/// After spawning, waits briefly and pings again so callers know whether the
/// daemon came up responsive or still needs time to initialize.
pub async fn ensure_daemon(
	client: &CoreClient,
	launch: &DaemonLaunchConfig,
) -> Result<EnsureDaemonOutcome> {
	if is_daemon_running(client).await {
		return Ok(EnsureDaemonOutcome::AlreadyRunning);
	}

	let mut command = launch.command()?;
	command.stdout(Stdio::null());
	command.stderr(Stdio::null());

	let child = command
		.spawn()
		.with_context(|| format!("failed to spawn daemon at {}", launch.daemon_path.display()))?;
	let pid = child.id();

	// Give the daemon a moment to bind its socket before verifying liveness
	tokio::time::sleep(Duration::from_millis(500)).await;

	Ok(EnsureDaemonOutcome::Started {
		pid,
		responsive: is_daemon_running(client).await,
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	use sd_core::infra::daemon::addr::instance_data_dir;

	/// A named instance's daemon resolves its directory from the base it is
	/// given; handed the instance's own directory it nested a second
	/// `instances/<name>` and its state left the CLI's view.
	#[test]
	fn the_daemon_resolves_the_instance_once() {
		let base = PathBuf::from("/data/spacedrive");
		let launch = DaemonLaunchConfig {
			daemon_path: PathBuf::from("sd-daemon"),
			base_data_dir: base.clone(),
			instance: Some("smoke".to_string()),
			no_default_sources: false,
		};
		let command = launch.command().unwrap();
		let args: Vec<_> = command.get_args().collect();
		assert_eq!(
			args,
			["--data-dir", "/data/spacedrive", "--instance", "smoke"].map(std::ffi::OsStr::new)
		);
		assert_eq!(
			instance_data_dir(PathBuf::from(args[1]), Some("smoke")),
			base.join("instances").join("smoke")
		);
	}
}
