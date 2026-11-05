use anyhow::Result;
use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand, Debug)]
pub enum DaemonCmd {
	/// Install daemon to start automatically on login
	Install,
	/// Uninstall daemon auto-start
	Uninstall,
	/// Check daemon auto-start status
	Status,
}

pub async fn run(
	data_dir: PathBuf,
	base_data_dir: PathBuf,
	instance: Option<String>,
	cmd: DaemonCmd,
) -> Result<()> {
	match cmd {
		DaemonCmd::Install => install_launchd_service(data_dir, base_data_dir, instance).await,
		DaemonCmd::Uninstall => uninstall_launchd_service(instance).await,
		DaemonCmd::Status => check_launchd_status(instance).await,
	}
}

const DOCS_URL: &str = "https://docs.spacedrive.com";

#[cfg(any(target_os = "linux", target_os = "macos", test))]
fn find_in_path(program: &str, path_var: &std::ffi::OsStr) -> Option<PathBuf> {
	std::env::split_paths(path_var)
		.map(|dir| dir.join(program))
		.find(|candidate| candidate.is_file())
}

/// Explains why systemd cannot run the daemon here, or `None` when it can.
///
/// A container or a non-systemd distribution has no `systemctl`, and a host
/// can carry the binary without booting systemd. `/run/systemd/system` is the
/// marker `sd_booted(3)` checks for the second case.
#[cfg(any(target_os = "linux", test))]
fn systemd_unavailable(
	path_var: &std::ffi::OsStr,
	run_dir: &std::path::Path,
) -> Option<&'static str> {
	if find_in_path("systemctl", path_var).is_none() {
		return Some("systemctl was not found on PATH, so this system does not use systemd");
	}
	if !run_dir.join("systemd/system").is_dir() {
		return Some("systemd is not running as the init system on this host");
	}
	None
}

#[cfg(any(target_os = "linux", target_os = "macos", test))]
fn no_service_manager_error(
	reason: &str,
	daemon_path: &std::path::Path,
	base_data_dir: &std::path::Path,
	instance: Option<&str>,
) -> anyhow::Error {
	let mut run = format!(
		"{} --data-dir {}",
		daemon_path.display(),
		base_data_dir.display()
	);
	if let Some(inst) = instance {
		run.push_str(&format!(" --instance {}", inst));
	}
	anyhow::anyhow!(
		"Cannot install the daemon as a service: {reason}.\n\n\
		Start the daemon yourself instead, with 'sd start' or directly:\n  {run}\n\n\
		To start it at boot, add that command to your init system or process supervisor. \
		See the deployment guide at {DOCS_URL}."
	)
}

#[cfg(target_os = "macos")]
async fn install_launchd_service(
	data_dir: PathBuf,
	base_data_dir: PathBuf,
	instance: Option<String>,
) -> Result<()> {
	use std::fs;
	use std::io::Write;

	let home =
		dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
	let launch_agents_dir = home.join("Library/LaunchAgents");

	// Create LaunchAgents directory if it doesn't exist
	fs::create_dir_all(&launch_agents_dir)?;

	// Determine plist filename based on instance
	let plist_name = if let Some(ref inst) = instance {
		format!("com.spacedrive.daemon.{}.plist", inst)
	} else {
		"com.spacedrive.daemon.plist".to_string()
	};
	let plist_path = launch_agents_dir.join(&plist_name);

	// Get the current daemon binary path
	let current_exe = std::env::current_exe()?;
	let daemon_path = current_exe
		.parent()
		.ok_or_else(|| anyhow::anyhow!("Could not determine binary directory"))?
		.join(sd_client::daemon_binary_name());

	if !daemon_path.exists() {
		return Err(anyhow::anyhow!(
			"Daemon binary not found at {}. Ensure both 'sd-cli' and 'sd-daemon' are in the same directory.",
			daemon_path.display()
		));
	}

	if find_in_path("launchctl", &std::env::var_os("PATH").unwrap_or_default()).is_none() {
		return Err(no_service_manager_error(
			"launchctl was not found on PATH",
			&daemon_path,
			&base_data_dir,
			instance.as_deref(),
		));
	}

	// Determine log paths
	let log_dir = data_dir.join("logs");
	fs::create_dir_all(&log_dir)?;
	let stdout_log = log_dir.join("daemon.out.log");
	let stderr_log = log_dir.join("daemon.err.log");

	// Build program arguments
	let mut program_args = vec![
		daemon_path.to_string_lossy().to_string(),
		"--data-dir".to_string(),
		base_data_dir.to_string_lossy().to_string(),
	];

	if let Some(ref inst) = instance {
		program_args.push("--instance".to_string());
		program_args.push(inst.clone());
	}

	// launchd starts agents with a 256 file-descriptor limit, which the daemon
	// warns about; SoftResourceLimits raises it for watchers and stores.
	let label = if let Some(ref inst) = instance {
		format!("com.spacedrive.daemon.{}", inst)
	} else {
		"com.spacedrive.daemon".to_string()
	};

	let plist_content = format!(
		r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Label</key>
	<string>{label}</string>
	<key>ProgramArguments</key>
	<array>
{program_args}
	</array>
	<key>RunAtLoad</key>
	<true/>
	<key>KeepAlive</key>
	<dict>
		<key>SuccessfulExit</key>
		<false/>
	</dict>
	<key>SoftResourceLimits</key>
	<dict>
		<key>NumberOfFiles</key>
		<integer>65536</integer>
	</dict>
	<key>StandardOutPath</key>
	<string>{stdout_log}</string>
	<key>StandardErrorPath</key>
	<string>{stderr_log}</string>
	<key>WorkingDirectory</key>
	<string>{working_dir}</string>
</dict>
</plist>
"#,
		label = label,
		program_args = program_args
			.iter()
			.map(|arg| format!("\t\t<string>{}</string>", arg))
			.collect::<Vec<_>>()
			.join("\n"),
		stdout_log = stdout_log.display(),
		stderr_log = stderr_log.display(),
		working_dir = home.display(),
	);

	// Write the plist file
	let mut file = fs::File::create(&plist_path)?;
	file.write_all(plist_content.as_bytes())?;

	println!("Created LaunchAgent: {}", plist_path.display());

	// Load the service
	let output = std::process::Command::new("launchctl")
		.arg("load")
		.arg(&plist_path)
		.output()?;

	if !output.status.success() {
		let stderr = String::from_utf8_lossy(&output.stderr);
		return Err(anyhow::anyhow!("Failed to load LaunchAgent: {}", stderr));
	}

	println!("Daemon installed successfully!");
	println!("The daemon will start automatically on login.");
	println!();
	println!("Logs:");
	println!("  stdout: {}", stdout_log.display());
	println!("  stderr: {}", stderr_log.display());

	Ok(())
}

#[cfg(target_os = "macos")]
async fn uninstall_launchd_service(instance: Option<String>) -> Result<()> {
	use std::fs;

	let home =
		dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
	let launch_agents_dir = home.join("Library/LaunchAgents");

	let plist_name = if let Some(ref inst) = instance {
		format!("com.spacedrive.daemon.{}.plist", inst)
	} else {
		"com.spacedrive.daemon.plist".to_string()
	};
	let plist_path = launch_agents_dir.join(&plist_name);

	if !plist_path.exists() {
		println!("Daemon auto-start is not installed.");
		return Ok(());
	}

	// Unload the service
	let output = std::process::Command::new("launchctl")
		.arg("unload")
		.arg(&plist_path)
		.output()?;

	// Don't fail if unload fails (service might not be running)
	if !output.status.success() {
		let stderr = String::from_utf8_lossy(&output.stderr);
		println!(
			"Warning: Failed to unload service (it may not be running): {}",
			stderr
		);
	}

	// Remove the plist file
	fs::remove_file(&plist_path)?;

	println!("Daemon auto-start uninstalled successfully!");

	Ok(())
}

#[cfg(target_os = "macos")]
async fn check_launchd_status(instance: Option<String>) -> Result<()> {
	let home =
		dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
	let launch_agents_dir = home.join("Library/LaunchAgents");

	let plist_name = if let Some(ref inst) = instance {
		format!("com.spacedrive.daemon.{}.plist", inst)
	} else {
		"com.spacedrive.daemon.plist".to_string()
	};
	let plist_path = launch_agents_dir.join(&plist_name);

	if !plist_path.exists() {
		println!("Daemon auto-start: Not installed");
		println!();
		println!("To install: sd-cli daemon install");
		return Ok(());
	}

	println!("Daemon auto-start: Installed");
	println!("LaunchAgent: {}", plist_path.display());

	// Check if the service is loaded
	let label = if let Some(ref inst) = instance {
		format!("com.spacedrive.daemon.{}", inst)
	} else {
		"com.spacedrive.daemon".to_string()
	};

	let output = std::process::Command::new("launchctl")
		.arg("list")
		.arg(&label)
		.output()?;

	if output.status.success() {
		let stdout = String::from_utf8_lossy(&output.stdout);
		println!("Service status: Loaded");

		// Parse PID from output if available
		if let Some(line) = stdout.lines().find(|l| l.contains("PID")) {
			println!("  {}", line.trim());
		}
	} else {
		println!("Service status: Not loaded");
		println!();
		println!("The service will start on next login, or run:");
		println!("  launchctl load {}", plist_path.display());
	}

	Ok(())
}

#[cfg(target_os = "linux")]
async fn install_launchd_service(
	_data_dir: PathBuf,
	base_data_dir: PathBuf,
	instance: Option<String>,
) -> Result<()> {
	use std::fs;
	use std::io::Write;

	let home =
		dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
	let systemd_user_dir = home.join(".config/systemd/user");

	// Determine service filename based on instance
	let service_name = if let Some(ref inst) = instance {
		format!("spacedrive-daemon@{}.service", inst)
	} else {
		"spacedrive-daemon.service".to_string()
	};
	let service_path = systemd_user_dir.join(&service_name);

	// Get the current daemon binary path
	let current_exe = std::env::current_exe()?;
	let daemon_path = current_exe
		.parent()
		.ok_or_else(|| anyhow::anyhow!("Could not determine binary directory"))?
		.join(sd_client::daemon_binary_name());

	if !daemon_path.exists() {
		return Err(anyhow::anyhow!(
			"Daemon binary not found at {}. Ensure both 'sd-cli' and 'sd-daemon' are in the same directory.",
			daemon_path.display()
		));
	}

	if let Some(reason) = systemd_unavailable(
		&std::env::var_os("PATH").unwrap_or_default(),
		std::path::Path::new("/run"),
	) {
		return Err(no_service_manager_error(
			reason,
			&daemon_path,
			&base_data_dir,
			instance.as_deref(),
		));
	}

	// A container or SSH session can have systemd as init but no user manager
	// to own a --user unit, so probe it before writing anything.
	let probe = std::process::Command::new("systemctl")
		.args(["--user", "show-environment"])
		.output()?;
	if !probe.status.success() {
		let stderr = String::from_utf8_lossy(&probe.stderr);
		return Err(no_service_manager_error(
			&format!(
				"the systemd user manager is not reachable ({})",
				stderr.trim()
			),
			&daemon_path,
			&base_data_dir,
			instance.as_deref(),
		));
	}

	// Build ExecStart command
	let mut exec_start = format!(
		"{} --data-dir {}",
		daemon_path.display(),
		base_data_dir.display()
	);
	if let Some(ref inst) = instance {
		exec_start.push_str(&format!(" --instance {}", inst));
	}

	// Build the systemd service unit file
	let service_content = format!(
		r#"[Unit]
Description=Spacedrive Daemon{}
After=network.target

[Service]
Type=simple
ExecStart={}
Restart=on-failure
RestartSec=5s
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=default.target
"#,
		if let Some(ref inst) = instance {
			format!(" ({})", inst)
		} else {
			String::new()
		},
		exec_start
	);

	fs::create_dir_all(&systemd_user_dir)?;
	let mut file = fs::File::create(&service_path)?;
	file.write_all(service_content.as_bytes())?;

	println!("Created systemd service: {}", service_path.display());

	// Reload systemd daemon
	let _ = std::process::Command::new("systemctl")
		.arg("--user")
		.arg("daemon-reload")
		.output();

	// Enable the service
	let output = std::process::Command::new("systemctl")
		.arg("--user")
		.arg("enable")
		.arg(&service_name)
		.output()?;

	if !output.status.success() {
		let stderr = String::from_utf8_lossy(&output.stderr);
		return Err(anyhow::anyhow!(
			"Failed to enable systemd service: {}",
			stderr
		));
	}

	// Start the service
	let output = std::process::Command::new("systemctl")
		.arg("--user")
		.arg("start")
		.arg(&service_name)
		.output()?;

	if !output.status.success() {
		let stderr = String::from_utf8_lossy(&output.stderr);
		return Err(anyhow::anyhow!(
			"Failed to start systemd service: {}",
			stderr
		));
	}

	println!("Daemon installed and started successfully!");
	println!("The daemon will start automatically on login.");
	println!();
	println!("Useful commands:");
	println!("  systemctl --user status {}", service_name);
	println!("  journalctl --user -u {} -f", service_name);

	Ok(())
}

#[cfg(target_os = "linux")]
async fn uninstall_launchd_service(instance: Option<String>) -> Result<()> {
	use std::fs;

	let home =
		dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
	let systemd_user_dir = home.join(".config/systemd/user");

	let service_name = if let Some(ref inst) = instance {
		format!("spacedrive-daemon@{}.service", inst)
	} else {
		"spacedrive-daemon.service".to_string()
	};
	let service_path = systemd_user_dir.join(&service_name);

	if !service_path.exists() {
		println!("Daemon auto-start is not installed.");
		return Ok(());
	}

	// Stop the service
	let _ = std::process::Command::new("systemctl")
		.arg("--user")
		.arg("stop")
		.arg(&service_name)
		.output();

	// Disable the service
	let _ = std::process::Command::new("systemctl")
		.arg("--user")
		.arg("disable")
		.arg(&service_name)
		.output();

	// Remove the service file
	fs::remove_file(&service_path)?;

	// Reload systemd daemon
	let _ = std::process::Command::new("systemctl")
		.arg("--user")
		.arg("daemon-reload")
		.output();

	println!("Daemon auto-start uninstalled successfully!");

	Ok(())
}

#[cfg(target_os = "linux")]
async fn check_launchd_status(instance: Option<String>) -> Result<()> {
	let home =
		dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
	let systemd_user_dir = home.join(".config/systemd/user");

	let service_name = if let Some(ref inst) = instance {
		format!("spacedrive-daemon@{}.service", inst)
	} else {
		"spacedrive-daemon.service".to_string()
	};
	let service_path = systemd_user_dir.join(&service_name);

	if !service_path.exists() {
		println!("Daemon auto-start: Not installed");
		println!();
		println!("To install: sd-cli daemon install");
		return Ok(());
	}

	println!("Daemon auto-start: Installed");
	println!("Service file: {}", service_path.display());
	println!();

	// Check service status
	let output = std::process::Command::new("systemctl")
		.arg("--user")
		.arg("is-active")
		.arg(&service_name)
		.output()
		.map_err(|e| anyhow::anyhow!("Could not run systemctl to read the service status: {e}"))?;

	let status = String::from_utf8_lossy(&output.stdout).trim().to_string();

	match status.as_str() {
		"active" => println!("Service status: ● Active (running)"),
		"inactive" => println!("Service status: ○ Inactive (stopped)"),
		"failed" => println!("Service status: Failed"),
		_ => println!("Service status: {}", status),
	}

	println!();
	println!("Useful commands:");
	println!("  systemctl --user status {}", service_name);
	println!("  systemctl --user start {}", service_name);
	println!("  systemctl --user stop {}", service_name);
	println!("  journalctl --user -u {} -f", service_name);

	Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
async fn install_launchd_service(
	_data_dir: PathBuf,
	_base_data_dir: PathBuf,
	_instance: Option<String>,
) -> Result<()> {
	Err(anyhow::anyhow!(
		"Daemon auto-start is currently only supported on macOS and Linux."
	))
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
async fn uninstall_launchd_service(_instance: Option<String>) -> Result<()> {
	Err(anyhow::anyhow!(
		"Daemon auto-start is currently only supported on macOS and Linux."
	))
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
async fn check_launchd_status(_instance: Option<String>) -> Result<()> {
	Err(anyhow::anyhow!(
		"Daemon auto-start is currently only supported on macOS and Linux."
	))
}

#[cfg(test)]
mod tests {
	use super::*;

	fn fake_bin(dir: &std::path::Path, name: &str) {
		std::fs::create_dir_all(dir).unwrap();
		std::fs::write(dir.join(name), b"").unwrap();
	}

	#[test]
	fn systemd_missing_when_systemctl_is_not_on_path() {
		let tmp = tempfile::tempdir().unwrap();
		let bin = tmp.path().join("bin");
		fake_bin(&bin, "ls");
		std::fs::create_dir_all(tmp.path().join("run/systemd/system")).unwrap();

		let reason = systemd_unavailable(bin.as_os_str(), &tmp.path().join("run"));
		assert!(reason.unwrap().contains("systemctl was not found"));
	}

	#[test]
	fn systemd_missing_when_not_booted_with_systemd() {
		let tmp = tempfile::tempdir().unwrap();
		let bin = tmp.path().join("bin");
		fake_bin(&bin, "systemctl");
		std::fs::create_dir_all(tmp.path().join("run")).unwrap();

		let reason = systemd_unavailable(bin.as_os_str(), &tmp.path().join("run"));
		assert!(reason.unwrap().contains("not running as the init system"));
	}

	#[test]
	fn systemd_available_with_systemctl_and_boot_marker() {
		let tmp = tempfile::tempdir().unwrap();
		let empty = tmp.path().join("empty");
		let bin = tmp.path().join("bin");
		std::fs::create_dir_all(&empty).unwrap();
		fake_bin(&bin, "systemctl");
		std::fs::create_dir_all(tmp.path().join("run/systemd/system")).unwrap();

		let path_var = std::env::join_paths([&empty, &bin]).unwrap();
		assert_eq!(
			systemd_unavailable(&path_var, &tmp.path().join("run")),
			None
		);
	}

	#[test]
	fn error_names_the_cause_and_the_command_to_run() {
		let message = no_service_manager_error(
			"systemctl was not found on PATH, so this system does not use systemd",
			std::path::Path::new("/opt/sd/sd-daemon"),
			std::path::Path::new("/data/spacedrive"),
			Some("work"),
		)
		.to_string();

		assert!(message.contains("systemctl was not found on PATH"));
		assert!(message.contains("/opt/sd/sd-daemon --data-dir /data/spacedrive --instance work"));
		assert!(message.contains("sd start"));
		assert!(message.contains(DOCS_URL));
	}
}
