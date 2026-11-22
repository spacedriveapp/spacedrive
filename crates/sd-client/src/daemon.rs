//! Daemon lifecycle helpers: liveness checks and background spawning.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::time::Duration;

use anyhow::{Context, Result};
use sd_core::infra::daemon::types::{DaemonRequest, DaemonResponse};
use tokio::net::TcpStream;
use tokio::time::{sleep, timeout, Instant};

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

/// How often the lifecycle waits below poll the daemon.
const POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Wait until nothing listens at `socket_addr`, returning false on timeout.
///
/// A daemon told to shut down keeps its listener bound until its core has
/// finished shutting down, and the kernel still completes connections in
/// that window. A refused connection is the first sign that the old process
/// is gone and a new daemon can bind the port.
pub async fn wait_for_daemon_exit(socket_addr: &str, limit: Duration) -> bool {
	let deadline = Instant::now() + limit;
	loop {
		// A connect that hangs means a full accept queue, so the daemon is still there.
		if let Ok(Err(_)) = timeout(Duration::from_secs(1), TcpStream::connect(socket_addr)).await {
			return true;
		}
		if Instant::now() >= deadline {
			return false;
		}
		sleep(POLL_INTERVAL).await;
	}
}

/// What became of a daemon started by [`spawn_daemon`].
#[derive(Debug)]
pub enum SpawnedDaemon {
	/// The daemon answered a ping.
	Ready { pid: u32 },
	/// The process exited before it answered.
	Exited { pid: u32, status: ExitStatus },
	/// The process is alive but never answered within the time limit.
	Unresponsive { pid: u32 },
}

/// Spawn the daemon in the background and wait until it answers a ping.
///
/// The daemon's stderr goes to `stderr_log`, truncated on each launch. An
/// error the daemon hits before its file logging starts, or returns from
/// main, only ever reaches stderr, so this file is what a caller points at
/// when the daemon dies.
pub async fn spawn_daemon(
	client: &CoreClient,
	launch: &DaemonLaunchConfig,
	stderr_log: &Path,
	limit: Duration,
) -> Result<SpawnedDaemon> {
	if let Some(dir) = stderr_log.parent() {
		tokio::fs::create_dir_all(dir)
			.await
			.with_context(|| format!("failed to create {}", dir.display()))?;
	}
	let stderr = tokio::fs::File::create(stderr_log)
		.await
		.with_context(|| format!("failed to create {}", stderr_log.display()))?
		.into_std()
		.await;

	let mut command = launch.command()?;
	command.stdout(Stdio::null());
	command.stderr(stderr);
	let mut child = command
		.spawn()
		.with_context(|| format!("failed to spawn daemon at {}", launch.daemon_path.display()))?;
	let pid = child.id();

	let deadline = Instant::now() + limit;
	loop {
		if let Some(status) = child.try_wait()? {
			return Ok(SpawnedDaemon::Exited { pid, status });
		}
		if let Ok(true) = timeout(Duration::from_secs(2), is_daemon_running(client)).await {
			return Ok(SpawnedDaemon::Ready { pid });
		}
		if Instant::now() >= deadline {
			return Ok(SpawnedDaemon::Unresponsive { pid });
		}
		sleep(POLL_INTERVAL).await;
	}
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

	/// The listener stays bound while a daemon shuts down, so the wait has
	/// to last until it is dropped, not a fixed sleep.
	#[tokio::test]
	async fn the_exit_wait_lasts_until_the_listener_closes() {
		let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
		let addr = listener.local_addr().unwrap().to_string();

		assert!(!wait_for_daemon_exit(&addr, Duration::from_millis(600)).await);

		tokio::spawn(async move {
			sleep(Duration::from_millis(800)).await;
			drop(listener);
		});
		assert!(wait_for_daemon_exit(&addr, Duration::from_secs(10)).await);
	}

	/// A daemon that dies on launch is reported with its exit status, and
	/// what it wrote to stderr is kept.
	#[cfg(unix)]
	#[tokio::test]
	async fn a_daemon_that_dies_on_launch_is_reported() {
		use std::os::unix::fs::PermissionsExt;

		let dir = tempfile::tempdir().unwrap();
		let daemon = dir.path().join("sd-daemon");
		std::fs::write(
			&daemon,
			"#!/bin/sh
echo 'Error: Address already in use' >&2
exit 1
",
		)
		.unwrap();
		std::fs::set_permissions(&daemon, std::fs::Permissions::from_mode(0o755)).unwrap();

		let unused = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
		let addr = unused.local_addr().unwrap().to_string();
		drop(unused);

		let launch = DaemonLaunchConfig {
			daemon_path: daemon,
			base_data_dir: dir.path().to_path_buf(),
			instance: None,
			no_default_sources: false,
		};
		let stderr_log = dir.path().join("logs").join("daemon-stderr.log");
		let outcome = spawn_daemon(
			&CoreClient::new(addr),
			&launch,
			&stderr_log,
			Duration::from_secs(10),
		)
		.await
		.unwrap();

		assert!(matches!(outcome, SpawnedDaemon::Exited { status, .. } if !status.success()));
		assert_eq!(
			std::fs::read_to_string(&stderr_log).unwrap(),
			"Error: Address already in use
"
		);
	}
}
