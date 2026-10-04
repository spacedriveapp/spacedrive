//! # Self-update
//!
//! `sd update` replaces the `sd` and `sd-daemon` binaries next to the running
//! executable with the ones published on GitHub for this platform. The stable
//! channel follows the repository's latest release and compares its tag with
//! the package version. The nightly channel follows the rolling `nightly`
//! prerelease that `.github/workflows/nightly.yml` moves along a branch, so
//! the package version never changes between builds and the updater compares
//! the commit the binary was built from (`SD_GIT_SHA`, shown by
//! `sd --version`) with the commit the release was tagged at.
//!
//! Nightly binaries are not signed with a Developer ID, so a checksum asset is
//! verified before anything is written. Replacement goes through a temporary
//! file and a rename, which keeps a running binary intact if the download or
//! the write fails part way.

mod timer;

use anyhow::{Context, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

use crate::config::CliConfig;

pub use timer::{run_timer, TimerCmd};

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const CURRENT_SHA: &str = env!("SD_GIT_SHA");
const NIGHTLY_CHANNEL: &str = "nightly";
const NIGHTLY_TAG: &str = "nightly";

/// Version string for `sd --version`: package version plus the build commit.
pub const VERSION_STRING: &str = concat!(env!("CARGO_PKG_VERSION"), " (", env!("SD_GIT_SHA"), ")");

#[derive(Debug, Deserialize)]
struct GitHubRelease {
	tag_name: String,
	#[serde(default)]
	target_commitish: String,
	assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
	name: String,
	browser_download_url: String,
	size: u64,
}

pub struct UpdateOptions {
	/// Reinstall even when the installed build already matches the release.
	pub force: bool,
	/// Skip the confirmation prompt, for timers and scripts.
	pub yes: bool,
}

pub async fn run(
	data_dir: PathBuf,
	instance: Option<String>,
	socket_addr: String,
	options: UpdateOptions,
) -> Result<()> {
	let config = CliConfig::load(&data_dir)?;
	let nightly = config.update.channel == NIGHTLY_CHANNEL;

	println!("Current version: {}", VERSION_STRING);
	println!("Update repository: {}", config.update.repo);
	println!("Update channel: {}", config.update.channel);
	println!();

	println!("Checking for updates...");
	let release = if nightly {
		fetch_release(&config.update.repo, &format!("tags/{}", NIGHTLY_TAG)).await?
	} else {
		fetch_release(&config.update.repo, "latest").await?
	};

	let (installed, available, up_to_date) = if nightly {
		let remote_sha = release.target_commitish.trim().to_string();
		if !looks_like_sha(&remote_sha) {
			return Err(anyhow::anyhow!(
				"Nightly release '{}' does not name the commit it was built from (target_commitish = '{}'); the workflow must create it with --target <sha>",
				release.tag_name,
				remote_sha
			));
		}
		let same = sha_matches(CURRENT_SHA, &remote_sha);
		(short_sha(CURRENT_SHA), short_sha(&remote_sha), same)
	} else {
		let latest = release.tag_name.trim_start_matches('v').to_string();
		let same = latest == CURRENT_VERSION;
		(CURRENT_VERSION.to_string(), latest, same)
	};

	println!("Available: {}", available);

	if up_to_date && !options.force {
		if nightly {
			println!(
				"You are already on the latest nightly build ({}).",
				installed
			);
		} else {
			println!("You are already on the latest version!");
		}
		return Ok(());
	}

	if !options.force && !options.yes {
		println!();
		println!("Update available: {} -> {}", installed, available);
		println!("Do you want to update? (y/N)");

		let mut response = String::new();
		std::io::stdin().read_line(&mut response)?;

		if !response.trim().eq_ignore_ascii_case("y") {
			println!("Update cancelled.");
			return Ok(());
		}
	}

	let platform = get_platform_string();
	println!();
	println!("Platform: {}", platform);

	let sd_asset = find_binary_asset(&release.assets, &platform, "sd")
		.ok_or_else(|| anyhow::anyhow!("Could not find sd binary for platform: {}", platform))?;
	let daemon_asset =
		find_binary_asset(&release.assets, &platform, "sd-daemon").ok_or_else(|| {
			anyhow::anyhow!("Could not find sd-daemon binary for platform: {}", platform)
		})?;

	println!("Downloading {}...", sd_asset.name);
	let sd_data = download_verified(&release.assets, sd_asset).await?;
	println!("Downloading {}...", daemon_asset.name);
	let daemon_data = download_verified(&release.assets, daemon_asset).await?;

	// Replace the file that is actually running (it may be named sd or sd-cli)
	// and the daemon next to it.
	let current_exe = std::env::current_exe()?;
	let current_exe = current_exe.canonicalize().unwrap_or(current_exe);
	let bin_dir = current_exe
		.parent()
		.ok_or_else(|| anyhow::anyhow!("Could not determine binary directory"))?;
	let daemon_path = bin_dir.join("sd-daemon");

	println!();
	println!("Installing updates...");

	let client = sd_client::CoreClient::new(socket_addr);
	let daemon_was_running = sd_client::is_daemon_running(&client).await;
	if daemon_was_running {
		println!("Stopping daemon...");
		client
			.send_raw_request(&sd_core::infra::daemon::types::DaemonRequest::Shutdown)
			.await?;
		tokio::time::sleep(std::time::Duration::from_secs(2)).await;
	}

	replace_binary(&current_exe, &sd_data)?;
	replace_binary(&daemon_path, &daemon_data)?;

	println!("Update complete!");

	if daemon_was_running {
		println!("Starting daemon...");
		restart_daemon(&client, &daemon_path, &data_dir, instance.as_deref()).await?;
	}

	println!();
	println!("Successfully updated to {}", available);

	Ok(())
}

/// Starts the daemon again after its binary changed.
///
/// When launchd owns the daemon, `kickstart -k` restarts it under launchd so it
/// keeps the agent's restart policy; spawning it ourselves would leave an
/// orphan the agent no longer tracks. Everywhere else the daemon is spawned the
/// same way `sd start` does.
async fn restart_daemon(
	client: &sd_client::CoreClient,
	daemon_path: &Path,
	data_dir: &Path,
	instance: Option<&str>,
) -> Result<()> {
	#[cfg(target_os = "macos")]
	{
		if let Some(label) = timer::loaded_daemon_agent(instance) {
			let status = std::process::Command::new("/bin/launchctl")
				.args([
					"kickstart",
					"-k",
					&format!("gui/{}/{}", timer::uid(), label),
				])
				.status()?;
			if status.success() {
				tokio::time::sleep(std::time::Duration::from_millis(500)).await;
				return Ok(());
			}
			println!("launchctl kickstart failed, starting the daemon directly");
		}
	}

	let launch = sd_client::DaemonLaunchConfig {
		daemon_path: daemon_path.to_path_buf(),
		data_dir: data_dir.to_path_buf(),
		instance: instance.map(str::to_string),
		no_default_sources: false,
	};
	sd_client::ensure_daemon(client, &launch).await?;
	Ok(())
}

async fn fetch_release(repo: &str, selector: &str) -> Result<GitHubRelease> {
	let url = format!(
		"https://api.github.com/repos/{}/releases/{}",
		repo, selector
	);

	let client = reqwest::Client::builder()
		.user_agent("spacedrive-cli")
		.build()?;

	let response = client.get(&url).send().await?;

	if !response.status().is_success() {
		return Err(anyhow::anyhow!(
			"Failed to fetch release {} from {}: HTTP {}",
			selector,
			repo,
			response.status()
		));
	}

	let release: GitHubRelease = response.json().await?;
	Ok(release)
}

/// Downloads an asset and checks it against its `.sha256` sibling when the
/// release ships one.
async fn download_verified(assets: &[GitHubAsset], asset: &GitHubAsset) -> Result<Vec<u8>> {
	let data = download_file(&asset.browser_download_url, asset.size).await?;

	let checksum_name = format!("{}.sha256", asset.name);
	let Some(checksum_asset) = assets.iter().find(|a| a.name == checksum_name) else {
		println!(
			"  no {} published, skipping checksum verification",
			checksum_name
		);
		return Ok(data);
	};

	let checksum_text = download_file(&checksum_asset.browser_download_url, checksum_asset.size)
		.await
		.with_context(|| format!("downloading {}", checksum_name))?;
	let expected = parse_sha256(&String::from_utf8_lossy(&checksum_text))
		.ok_or_else(|| anyhow::anyhow!("{} does not contain a sha256 digest", checksum_name))?;

	let actual = Sha256::digest(&data)
		.iter()
		.map(|b| format!("{:02x}", b))
		.collect::<String>();

	if actual != expected {
		return Err(anyhow::anyhow!(
			"Checksum mismatch for {}: expected {}, got {}",
			asset.name,
			expected,
			actual
		));
	}

	println!("  sha256 verified");
	Ok(data)
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

/// Replaces `path` atomically: the new bytes land in a sibling temp file that
/// is renamed over the target, so a running binary is never truncated or
/// partially overwritten.
fn replace_binary(path: &Path, data: &[u8]) -> Result<()> {
	use std::fs;

	let file_name = path
		.file_name()
		.and_then(|n| n.to_str())
		.ok_or_else(|| anyhow::anyhow!("Invalid binary path: {}", path.display()))?;
	let staged = path.with_file_name(format!("{}.update", file_name));
	let backup = path.with_file_name(format!("{}.bak", file_name));

	fs::write(&staged, data).with_context(|| format!("writing {}", staged.display()))?;

	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt;
		fs::set_permissions(&staged, fs::Permissions::from_mode(0o755))?;
	}

	if path.exists() {
		fs::copy(path, &backup)?;
	}

	match fs::rename(&staged, path) {
		Ok(()) => {
			let _ = fs::remove_file(&backup);
			Ok(())
		}
		Err(e) => {
			if backup.exists() {
				let _ = fs::rename(&backup, path);
			}
			let _ = fs::remove_file(&staged);
			Err(e).with_context(|| format!("replacing {}", path.display()))
		}
	}
}

/// Picks the release asset for one binary on one platform.
///
/// Asset names are `<binary>-<platform>` with optional `.exe`; stripping the
/// platform leaves the binary name, so `sd` never matches `sd-daemon`. The
/// stable server tarball names the client `sd-cli`, which is the same binary.
fn find_binary_asset<'a>(
	assets: &'a [GitHubAsset],
	platform: &str,
	binary: &str,
) -> Option<&'a GitHubAsset> {
	assets.iter().find(|a| {
		let name = a.name.strip_suffix(".exe").unwrap_or(&a.name);
		let Some(stem) = name.strip_suffix(platform) else {
			return false;
		};
		let stem = stem.trim_end_matches(['-', '_']);
		match binary {
			"sd" => stem == "sd" || stem == "sd-cli",
			other => stem == other,
		}
	})
}

fn parse_sha256(text: &str) -> Option<String> {
	let token = text.split_whitespace().next()?;
	(token.len() == 64 && token.chars().all(|c| c.is_ascii_hexdigit()))
		.then(|| token.to_ascii_lowercase())
}

fn looks_like_sha(s: &str) -> bool {
	s.len() >= 7 && s.len() <= 40 && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// Equal when one sha is a prefix of the other, so an abbreviated commit still
/// matches the full one.
fn sha_matches(a: &str, b: &str) -> bool {
	let a = a.to_ascii_lowercase();
	let b = b.to_ascii_lowercase();
	looks_like_sha(&a) && looks_like_sha(&b) && (a.starts_with(&b) || b.starts_with(&a))
}

fn short_sha(sha: &str) -> String {
	sha.chars().take(12).collect()
}

/// Platform suffix shared by the release workflows and the updater.
pub fn get_platform_string() -> String {
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

#[cfg(test)]
mod tests {
	use super::*;

	fn asset(name: &str) -> GitHubAsset {
		GitHubAsset {
			name: name.to_string(),
			browser_download_url: String::new(),
			size: 0,
		}
	}

	#[test]
	fn sd_and_daemon_resolve_to_distinct_assets() {
		let assets = vec![
			asset("sd-daemon-macos-aarch64"),
			asset("sd-daemon-macos-aarch64.sha256"),
			asset("sd-macos-aarch64"),
			asset("sd-macos-aarch64.sha256"),
			asset("sd-linux-x86_64"),
		];

		let sd = find_binary_asset(&assets, "macos-aarch64", "sd").unwrap();
		let daemon = find_binary_asset(&assets, "macos-aarch64", "sd-daemon").unwrap();
		assert_eq!(sd.name, "sd-macos-aarch64");
		assert_eq!(daemon.name, "sd-daemon-macos-aarch64");
		assert!(find_binary_asset(&assets, "windows-x86_64", "sd").is_none());
	}

	#[test]
	fn stable_names_and_windows_suffix_still_match() {
		let assets = vec![
			asset("sd-cli-linux-x86_64"),
			asset("sd-daemon-windows-x86_64.exe"),
			asset("sd-windows-x86_64.exe"),
		];
		assert_eq!(
			find_binary_asset(&assets, "linux-x86_64", "sd")
				.unwrap()
				.name,
			"sd-cli-linux-x86_64"
		);
		assert_eq!(
			find_binary_asset(&assets, "windows-x86_64", "sd")
				.unwrap()
				.name,
			"sd-windows-x86_64.exe"
		);
		assert_eq!(
			find_binary_asset(&assets, "windows-x86_64", "sd-daemon")
				.unwrap()
				.name,
			"sd-daemon-windows-x86_64.exe"
		);
	}

	#[test]
	fn sha_comparison_accepts_prefixes() {
		assert!(sha_matches(
			"46a54ca0000000000000000000000000deadbeef",
			"46a54ca"
		));
		assert!(!sha_matches("46a54ca", "ebee7ae"));
		assert!(!sha_matches("unknown", "46a54ca"));
	}

	#[test]
	fn checksum_files_parse_first_token() {
		assert_eq!(
			parse_sha256(&format!("{}  sd-macos-aarch64\n", "a".repeat(64))).as_deref(),
			Some("a".repeat(64).as_str())
		);
		assert!(parse_sha256("not a digest").is_none());
	}
}
