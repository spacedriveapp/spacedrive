use anyhow::Result;
use serde::Deserialize;
use std::path::PathBuf;

use crate::config::CliConfig;

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Deserialize)]
struct GitHubRelease {
	tag_name: String,
	name: String,
	assets: Vec<GitHubAsset>,
	prerelease: bool,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
	name: String,
	browser_download_url: String,
	size: u64,
}

pub async fn run(data_dir: PathBuf, force: bool) -> Result<()> {
	let config = CliConfig::load(&data_dir)?;

	println!("Current version: {}", CURRENT_VERSION);
	println!("Update repository: {}", config.update.repo);
	println!("Update channel: {}", config.update.channel);
	println!();

	// Check for updates
	println!("Checking for updates...");
	let latest_release = fetch_latest_release(&config.update.repo).await?;

	let latest_version = latest_release.tag_name.trim_start_matches('v');
	println!("Latest version: {}", latest_version);

	if latest_version == CURRENT_VERSION && !force {
		println!("You are already on the latest version!");
		return Ok(());
	}

	if !force {
		println!();
		println!(
			"Update available: {} -> {}",
			CURRENT_VERSION, latest_version
		);
		println!("Do you want to update? (y/N)");

		let mut response = String::new();
		std::io::stdin().read_line(&mut response)?;

		if !response.trim().eq_ignore_ascii_case("y") {
			println!("Update cancelled.");
			return Ok(());
		}
	}

	// Determine platform
	let platform = get_platform_string();
	println!();
	println!("Platform: {}", platform);

	// Find matching assets
	let sd_asset = latest_release
		.assets
		.iter()
		.find(|a| a.name.contains(&platform) && a.name.contains("sd"))
		.ok_or_else(|| anyhow::anyhow!("Could not find sd binary for platform: {}", platform))?;

	let daemon_asset = latest_release
		.assets
		.iter()
		.find(|a| a.name.contains(&platform) && a.name.contains("sd-daemon"))
		.ok_or_else(|| {
			anyhow::anyhow!("Could not find sd-daemon binary for platform: {}", platform)
		})?;

	println!("Downloading updates...");

	// Download binaries
	let sd_data = download_file(&sd_asset.browser_download_url, sd_asset.size).await?;
	let daemon_data = download_file(&daemon_asset.browser_download_url, daemon_asset.size).await?;

	// Get current binary paths
	let current_exe = std::env::current_exe()?;
	let bin_dir = current_exe
		.parent()
		.ok_or_else(|| anyhow::anyhow!("Could not determine binary directory"))?;

	let sd_path = bin_dir.join("sd");
	let daemon_path = bin_dir.join("sd-daemon");

	println!();
	println!("Installing updates...");

	// Check if daemon is running
	let daemon_was_running = check_daemon_running(&data_dir).await;
	if daemon_was_running {
		println!("Stopping daemon...");
		stop_daemon(&data_dir).await?;
	}

	// Perform atomic replacement
	replace_binary(&sd_path, &sd_data)?;
	replace_binary(&daemon_path, &daemon_data)?;

	println!("Update complete!");

	if daemon_was_running {
		println!("Starting daemon...");
		start_daemon(&data_dir).await?;
	}

	println!();
	println!("Successfully updated to version {}", latest_version);

	Ok(())
}

async fn fetch_latest_release(repo: &str) -> Result<GitHubRelease> {
	let url = format!("https://api.github.com/repos/{}/releases/latest", repo);

	let client = reqwest::Client::builder()
		.user_agent("spacedrive-cli")
		.build()?;

	let response = client.get(&url).send().await?;

	if !response.status().is_success() {
		return Err(anyhow::anyhow!(
			"Failed to fetch releases: HTTP {}",
			response.status()
		));
	}

	let release: GitHubRelease = response.json().await?;
	Ok(release)
}

async fn download_file(url: &str, expected_size: u64) -> Result<Vec<u8>> {
	let client = reqwest::Client::builder()
		.user_agent("spacedrive-cli")
		.build()?;

	let response = client.get(url).send().await?;

	if !response.status().is_success() {
		return Err(anyhow::anyhow!(
			"Failed to download: HTTP {}",
			response.status()
		));
	}

	let bytes = response.bytes().await?;

	if bytes.len() as u64 != expected_size {
		return Err(anyhow::anyhow!(
			"Downloaded file size mismatch: expected {}, got {}",
			expected_size,
			bytes.len()
		));
	}

	Ok(bytes.to_vec())
}

fn replace_binary(path: &PathBuf, data: &[u8]) -> Result<()> {
	use std::fs;

	// Create backup
	let backup_path = path.with_extension("bak");
	if path.exists() {
		fs::copy(path, &backup_path)?;
	}

	// Write new binary
	match fs::write(path, data) {
		Ok(()) => {
			// Set executable permissions (Unix only)
			// We guard this block so it is not compiled on Windows
			#[cfg(unix)]
			{
				use std::os::unix::fs::PermissionsExt;
				let mut perms = fs::metadata(path)?.permissions();
				perms.set_mode(0o755);
				fs::set_permissions(path, perms)?;
			}

			// Remove backup on success
			if backup_path.exists() {
				let _ = fs::remove_file(&backup_path);
			}

			Ok(())
		}
		Err(e) => {
			// Restore from backup on failure
			if backup_path.exists() {
				let _ = fs::copy(&backup_path, path);
				let _ = fs::remove_file(&backup_path);
			}
			Err(e.into())
		}
	}
}

fn get_platform_string() -> String {
	let os = std::env::consts::OS;
	let arch = std::env::consts::ARCH;

	match (os, arch) {
		("macos", "aarch64") => "macos-aarch64".to_string(),
		("macos", "x86_64") => "macos-x86_64".to_string(),
		("linux", "x86_64") => "linux-x86_64".to_string(),
		("linux", "aarch64") => "linux-aarch64".to_string(),
		("windows", "x86_64") => "windows-x86_64".to_string(),
		_ => {
			eprintln!(
				"Warning: Unsupported platform {}-{}, trying anyway...",
				os, arch
			);
			format!("{}-{}", os, arch)
		}
	}
}

async fn check_daemon_running(data_dir: &PathBuf) -> bool {
	let socket_addr = "127.0.0.1:6969".to_string();
	let client = sd_core::client::CoreClient::new(socket_addr);

	matches!(
		client
			.send_raw_request(&sd_core::infra::daemon::types::DaemonRequest::Ping)
			.await,
		Ok(sd_core::infra::daemon::types::DaemonResponse::Pong)
	)
}

async fn stop_daemon(data_dir: &PathBuf) -> Result<()> {
	let socket_addr = "127.0.0.1:6969".to_string();
	let client = sd_core::client::CoreClient::new(socket_addr);

	client
		.send_raw_request(&sd_core::infra::daemon::types::DaemonRequest::Shutdown)
		.await?;

	// Wait for shutdown
	tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

	Ok(())
}

async fn start_daemon(data_dir: &PathBuf) -> Result<()> {
	let current_exe = std::env::current_exe()?;
	let daemon_path = current_exe.parent().unwrap().join("sd-daemon");

	let mut command = std::process::Command::new(daemon_path);
	command.arg("--data-dir").arg(data_dir);
	command.stdout(std::process::Stdio::null());
	command.stderr(std::process::Stdio::null());

	command.spawn()?;

	// Wait for startup
	tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

	Ok(())
}