//! Periodic `sd update --yes` as a user service: a launchd agent on macOS and a
//! systemd user timer on Linux, mirroring `sd daemon install`.

use anyhow::Result;
use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand, Debug)]
pub enum TimerCmd {
	/// Install a user service that runs `sd update --yes` periodically
	InstallTimer {
		/// Seconds between update checks
		#[arg(long, default_value_t = 3600)]
		interval: u32,
	},
	/// Remove the periodic update service
	UninstallTimer,
	/// Show whether the periodic update service is installed
	TimerStatus,
}

pub async fn run_timer(data_dir: PathBuf, instance: Option<String>, cmd: TimerCmd) -> Result<()> {
	match cmd {
		TimerCmd::InstallTimer { interval } => install(data_dir, instance, interval),
		TimerCmd::UninstallTimer => uninstall(instance),
		TimerCmd::TimerStatus => status(instance),
	}
}

fn label(instance: Option<&str>) -> String {
	match instance {
		Some(inst) => format!("com.spacedrive.update.{}", inst),
		None => "com.spacedrive.update".to_string(),
	}
}

/// The command the service runs. The absolute path of the running executable
/// is used because launchd and systemd start services with a minimal PATH.
fn update_command(data_dir: &PathBuf, instance: Option<&str>) -> Result<Vec<String>> {
	let exe = std::env::current_exe()?;
	let exe = exe.canonicalize().unwrap_or(exe);
	let mut args = vec![
		exe.to_string_lossy().to_string(),
		"--data-dir".to_string(),
		data_dir.to_string_lossy().to_string(),
	];
	if let Some(inst) = instance {
		args.push("--instance".to_string());
		args.push(inst.to_string());
	}
	args.push("update".to_string());
	args.push("--yes".to_string());
	Ok(args)
}

#[cfg(target_os = "macos")]
pub fn uid() -> u32 {
	// SAFETY: getuid has no preconditions and cannot fail.
	unsafe { libc::getuid() }
}

#[cfg(target_os = "macos")]
fn launch_agent_path(name: &str) -> Result<PathBuf> {
	let home =
		dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
	Ok(home
		.join("Library/LaunchAgents")
		.join(format!("{}.plist", name)))
}

/// Label of the daemon launch agent `sd daemon install` wrote, when launchd
/// has it loaded, so an update can restart the daemon through launchd.
#[cfg(target_os = "macos")]
pub fn loaded_daemon_agent(instance: Option<&str>) -> Option<String> {
	let label = match instance {
		Some(inst) => format!("com.spacedrive.daemon.{}", inst),
		None => "com.spacedrive.daemon".to_string(),
	};
	if !launch_agent_path(&label).ok()?.exists() {
		return None;
	}
	let loaded = std::process::Command::new("/bin/launchctl")
		.args(["list", &label])
		.output()
		.map(|o| o.status.success())
		.unwrap_or(false);
	loaded.then_some(label)
}

#[cfg(target_os = "macos")]
fn install(data_dir: PathBuf, instance: Option<String>, interval: u32) -> Result<()> {
	let label = label(instance.as_deref());
	let plist_path = launch_agent_path(&label)?;
	std::fs::create_dir_all(plist_path.parent().unwrap())?;

	let log_dir = data_dir.join("logs");
	std::fs::create_dir_all(&log_dir)?;
	let log_path = log_dir.join("update.log");

	let program_args = update_command(&data_dir, instance.as_deref())?
		.iter()
		.map(|arg| format!("\t\t<string>{}</string>", arg))
		.collect::<Vec<_>>()
		.join("\n");

	let plist = format!(
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
	<key>StartInterval</key>
	<integer>{interval}</integer>
	<key>StandardOutPath</key>
	<string>{log}</string>
	<key>StandardErrorPath</key>
	<string>{log}</string>
</dict>
</plist>
"#,
		label = label,
		program_args = program_args,
		interval = interval,
		log = log_path.display(),
	);

	// Reload so a changed interval or binary path takes effect.
	if plist_path.exists() {
		let _ = std::process::Command::new("/bin/launchctl")
			.args(["unload", &plist_path.to_string_lossy()])
			.output();
	}
	std::fs::write(&plist_path, plist)?;
	println!("Created LaunchAgent: {}", plist_path.display());

	let output = std::process::Command::new("/bin/launchctl")
		.args(["load", &plist_path.to_string_lossy()])
		.output()?;
	if !output.status.success() {
		return Err(anyhow::anyhow!(
			"Failed to load LaunchAgent: {}",
			String::from_utf8_lossy(&output.stderr)
		));
	}

	println!(
		"Update timer installed: `sd update --yes` runs now and every {} seconds.",
		interval
	);
	println!("Log: {}", log_path.display());
	Ok(())
}

#[cfg(target_os = "macos")]
fn uninstall(instance: Option<String>) -> Result<()> {
	let plist_path = launch_agent_path(&label(instance.as_deref()))?;
	if !plist_path.exists() {
		println!("Update timer is not installed.");
		return Ok(());
	}
	let _ = std::process::Command::new("/bin/launchctl")
		.args(["unload", &plist_path.to_string_lossy()])
		.output();
	std::fs::remove_file(&plist_path)?;
	println!("Update timer uninstalled.");
	Ok(())
}

#[cfg(target_os = "macos")]
fn status(instance: Option<String>) -> Result<()> {
	let label = label(instance.as_deref());
	let plist_path = launch_agent_path(&label)?;
	if !plist_path.exists() {
		println!("Update timer: Not installed");
		println!("To install: sd update install-timer");
		return Ok(());
	}
	println!("Update timer: Installed");
	println!("LaunchAgent: {}", plist_path.display());
	let loaded = std::process::Command::new("/bin/launchctl")
		.args(["list", &label])
		.output()
		.map(|o| o.status.success())
		.unwrap_or(false);
	println!(
		"Service status: {}",
		if loaded { "Loaded" } else { "Not loaded" }
	);
	Ok(())
}

#[cfg(target_os = "linux")]
fn unit_paths(instance: Option<&str>) -> Result<(PathBuf, String)> {
	let home =
		dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
	let name = match instance {
		Some(inst) => format!("spacedrive-update@{}", inst),
		None => "spacedrive-update".to_string(),
	};
	Ok((home.join(".config/systemd/user"), name))
}

#[cfg(target_os = "linux")]
fn install(data_dir: PathBuf, instance: Option<String>, interval: u32) -> Result<()> {
	let (dir, name) = unit_paths(instance.as_deref())?;
	std::fs::create_dir_all(&dir)?;

	let exec_start = update_command(&data_dir, instance.as_deref())?.join(" ");
	std::fs::write(
		dir.join(format!("{}.service", name)),
		format!(
			"[Unit]\nDescription=Spacedrive update check\n\n[Service]\nType=oneshot\nExecStart={}\n",
			exec_start
		),
	)?;
	std::fs::write(
		dir.join(format!("{}.timer", name)),
		format!(
			"[Unit]\nDescription=Spacedrive update check\n\n[Timer]\nOnBootSec=1min\nOnUnitActiveSec={}s\nPersistent=true\n\n[Install]\nWantedBy=timers.target\n",
			interval
		),
	)?;

	let _ = std::process::Command::new("systemctl")
		.args(["--user", "daemon-reload"])
		.output();
	let output = std::process::Command::new("systemctl")
		.args(["--user", "enable", "--now", &format!("{}.timer", name)])
		.output()?;
	if !output.status.success() {
		return Err(anyhow::anyhow!(
			"Failed to enable timer: {}",
			String::from_utf8_lossy(&output.stderr)
		));
	}

	println!(
		"Update timer installed: `sd update --yes` runs every {} seconds ({}.timer).",
		interval, name
	);
	Ok(())
}

#[cfg(target_os = "linux")]
fn uninstall(instance: Option<String>) -> Result<()> {
	let (dir, name) = unit_paths(instance.as_deref())?;
	let timer = dir.join(format!("{}.timer", name));
	if !timer.exists() {
		println!("Update timer is not installed.");
		return Ok(());
	}
	let _ = std::process::Command::new("systemctl")
		.args(["--user", "disable", "--now", &format!("{}.timer", name)])
		.output();
	let _ = std::fs::remove_file(&timer);
	let _ = std::fs::remove_file(dir.join(format!("{}.service", name)));
	let _ = std::process::Command::new("systemctl")
		.args(["--user", "daemon-reload"])
		.output();
	println!("Update timer uninstalled.");
	Ok(())
}

#[cfg(target_os = "linux")]
fn status(instance: Option<String>) -> Result<()> {
	let (dir, name) = unit_paths(instance.as_deref())?;
	if !dir.join(format!("{}.timer", name)).exists() {
		println!("Update timer: Not installed");
		println!("To install: sd update install-timer");
		return Ok(());
	}
	println!("Update timer: Installed ({}.timer)", name);
	let output = std::process::Command::new("systemctl")
		.args(["--user", "is-active", &format!("{}.timer", name)])
		.output()?;
	println!(
		"Service status: {}",
		String::from_utf8_lossy(&output.stdout).trim()
	);
	Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn install(_data_dir: PathBuf, _instance: Option<String>, _interval: u32) -> Result<()> {
	Err(anyhow::anyhow!(
		"The update timer is only available on macOS (launchd) and Linux (systemd)"
	))
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn uninstall(_instance: Option<String>) -> Result<()> {
	println!("Update timer is not supported on this platform.");
	Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn status(_instance: Option<String>) -> Result<()> {
	println!("Update timer is not supported on this platform.");
	Ok(())
}
