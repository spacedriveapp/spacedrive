//! # Self-update
//!
//! `sd update` replaces the `sd` and `sd-daemon` binaries next to the running
//! executable with the ones published on GitHub for this platform. The stable
//! channel follows the repository's latest release and compares its tag with
//! the package version. The nightly channel follows the rolling `nightly`
//! prerelease that `.github/workflows/nightly.yml` moves along a branch, so
//! the package version never changes between builds and the updater compares
//! the commit the binary was built from (`SD_GIT_SHA`, shown by
//! `sd --version`) with the commit the `nightly` tag points at. That commit
//! comes from the git ref API rather than the release's `target_commitish`,
//! which GitHub freezes when the release is created and the workflow edits
//! the release in place; when the ref API is rate limited the `nightly.sha`
//! asset the workflow uploads says the same thing.
//!
//! Nightly binaries are not signed with a Developer ID, so a checksum asset is
//! verified before anything is written. Replacement goes through a temporary
//! file and a rename, which keeps a running binary intact if the download or
//! the write fails part way.
//!
//! Downloads land in `<data dir>/updates/<asset>.<build>.part` and resume
//! from the part's length with a `Range` request, so a timer tick on a link
//! that drops mid-transfer keeps what it got for the next tick instead of
//! starting over every hour. Every attempt, however it ends, appends a line
//! to `<data dir>/logs/update.log` and records itself in
//! `<data dir>/update-state.json`, which `sd update status` reads back.
//! After an install the build's commit (or release tag) is written to
//! `sd-version.txt` beside the binaries, the same marker a local rebuild
//! script writes.

mod timer;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::config::CliConfig;

pub use timer::{run_timer, TimerCmd};

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const CURRENT_SHA: &str = env!("SD_GIT_SHA");
const NIGHTLY_CHANNEL: &str = "nightly";
const NIGHTLY_TAG: &str = "nightly";
/// Asset naming the commit the nightly tag points at, for when the git ref
/// API answers with a rate limit.
const NIGHTLY_SHA_ASSET: &str = "nightly.sha";
const GITHUB_API: &str = "https://api.github.com";
/// Marker beside the binaries naming the build they came from.
const VERSION_FILE: &str = "sd-version.txt";
const STATE_FILE: &str = "update-state.json";
/// A download that moves no bytes for this long is abandoned for this tick;
/// the part on disk is kept for the next one.
const DOWNLOAD_IDLE_TIMEOUT: Duration = Duration::from_secs(60);

/// Version string for `sd --version`: package version plus the build commit.
pub const VERSION_STRING: &str = concat!(env!("CARGO_PKG_VERSION"), " (", env!("SD_GIT_SHA"), ")");

#[derive(Debug, Deserialize)]
struct GitHubRelease {
	tag_name: String,
	assets: Vec<GitHubAsset>,
}

/// A git ref or annotated tag from the API: the object it points at.
#[derive(Debug, Deserialize)]
struct GitHubRefObject {
	object: GitHubObject,
}

#[derive(Debug, Deserialize)]
struct GitHubObject {
	sha: String,
	#[serde(rename = "type")]
	kind: String,
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

/// What one `sd update` run ended as, for the log and the state file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "result")]
pub enum AttemptResult {
	UpToDate,
	Cancelled,
	Updated,
	Failed { reason: String },
}

/// The last `sd update` run, persisted so `sd update status` can show it
/// without digging through the log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateState {
	pub last_attempt: DateTime<Utc>,
	pub channel: String,
	pub installed: String,
	pub available: Option<String>,
	#[serde(flatten)]
	pub result: AttemptResult,
}

impl UpdateState {
	fn load(data_dir: &Path) -> Option<Self> {
		let text = std::fs::read_to_string(data_dir.join(STATE_FILE)).ok()?;
		serde_json::from_str(&text).ok()
	}

	fn summary(&self) -> String {
		match &self.result {
			AttemptResult::UpToDate => format!("up to date ({})", self.installed),
			AttemptResult::Cancelled => "cancelled at the prompt".to_string(),
			AttemptResult::Updated => format!(
				"updated {} -> {}",
				self.installed,
				self.available.as_deref().unwrap_or("?")
			),
			AttemptResult::Failed { reason } => format!("failed: {}", reason),
		}
	}
}

pub async fn run(
	data_dir: PathBuf,
	instance: Option<String>,
	socket_addr: String,
	options: UpdateOptions,
) -> Result<()> {
	let config = CliConfig::load(&data_dir)?;
	let nightly = config.update.channel == NIGHTLY_CHANNEL;
	let installed = if nightly {
		short_sha(CURRENT_SHA)
	} else {
		CURRENT_VERSION.to_string()
	};

	let mut available = None;
	let outcome = attempt(
		&data_dir,
		instance,
		socket_addr,
		&config,
		&options,
		&mut available,
	)
	.await;

	let result = match &outcome {
		Ok(result) => result.clone(),
		Err(e) => AttemptResult::Failed {
			reason: format!("{:#}", e),
		},
	};
	let state = UpdateState {
		last_attempt: Utc::now(),
		channel: config.update.channel.clone(),
		installed,
		available,
		result,
	};
	if let Err(e) = record_attempt(&data_dir, &state) {
		eprintln!("Warning: could not record the update attempt: {}", e);
	}

	outcome.map(|_| ())
}

/// Appends the attempt to update.log and replaces the state file.
fn record_attempt(data_dir: &Path, state: &UpdateState) -> Result<()> {
	let log_dir = data_dir.join("logs");
	std::fs::create_dir_all(&log_dir)?;
	let mut log = std::fs::OpenOptions::new()
		.create(true)
		.append(true)
		.open(log_dir.join("update.log"))?;
	writeln!(
		log,
		"{} update channel={} installed={} available={} {}",
		state
			.last_attempt
			.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
		state.channel,
		state.installed,
		state.available.as_deref().unwrap_or("-"),
		state.summary()
	)?;

	let staged = data_dir.join(format!("{}.tmp", STATE_FILE));
	std::fs::write(&staged, serde_json::to_vec_pretty(state)?)?;
	std::fs::rename(&staged, data_dir.join(STATE_FILE))?;
	Ok(())
}

async fn attempt(
	data_dir: &Path,
	instance: Option<String>,
	socket_addr: String,
	config: &CliConfig,
	options: &UpdateOptions,
	available_out: &mut Option<String>,
) -> Result<AttemptResult> {
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

	let nightly_sha = if nightly {
		Some(resolve_nightly_sha(GITHUB_API, &config.update.repo, &release).await?)
	} else {
		None
	};

	let (installed, available, up_to_date) = if let Some(remote_sha) = &nightly_sha {
		let same = sha_matches(CURRENT_SHA, remote_sha);
		(short_sha(CURRENT_SHA), short_sha(remote_sha), same)
	} else {
		let latest = release.tag_name.trim_start_matches('v').to_string();
		(
			CURRENT_VERSION.to_string(),
			latest.clone(),
			!is_newer_version(&latest, CURRENT_VERSION),
		)
	};

	println!("Available: {}", available);
	*available_out = Some(available.clone());

	if up_to_date && !options.force {
		if nightly {
			println!(
				"You are already on the latest nightly build ({}).",
				installed
			);
		} else {
			println!(
				"No newer release than {} is published; you are up to date.",
				installed
			);
		}
		return Ok(AttemptResult::UpToDate);
	}

	if !options.force && !options.yes {
		println!();
		println!("Update available: {} -> {}", installed, available);
		println!("Do you want to update? (y/N)");

		let mut response = String::new();
		std::io::stdin().read_line(&mut response)?;

		if !response.trim().eq_ignore_ascii_case("y") {
			println!("Update cancelled.");
			return Ok(AttemptResult::Cancelled);
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

	// Parts are keyed by the build they belong to, so a tick that finds a
	// newer nightly does not resume into the previous one's bytes.
	let build_key = match &nightly_sha {
		Some(sha) => short_sha(sha),
		None => release.tag_name.trim_start_matches('v').to_string(),
	};
	let part_dir = data_dir.join("updates");
	discard_other_parts(&part_dir, &build_key);

	println!("Downloading {}...", sd_asset.name);
	let sd_data = download_verified(&release.assets, sd_asset, &part_dir, &build_key).await?;
	println!("Downloading {}...", daemon_asset.name);
	let daemon_data =
		download_verified(&release.assets, daemon_asset, &part_dir, &build_key).await?;

	// Replace the file that is actually running (it may be named sd or sd-cli)
	// and the daemon next to it.
	let current_exe = std::env::current_exe()?;
	let current_exe = current_exe.canonicalize().unwrap_or(current_exe);
	let bin_dir = current_exe
		.parent()
		.ok_or_else(|| anyhow::anyhow!("Could not determine binary directory"))?;
	let daemon_path = bin_dir.join(sd_client::daemon_binary_name());

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
	let _ = std::fs::remove_dir_all(&part_dir);

	let version_marker = nightly_sha.unwrap_or_else(|| release.tag_name.clone());
	if let Err(e) = std::fs::write(bin_dir.join(VERSION_FILE), format!("{}\n", version_marker)) {
		println!(
			"  could not write {}: {}",
			bin_dir.join(VERSION_FILE).display(),
			e
		);
	}

	println!("Update complete!");

	if daemon_was_running {
		println!("Starting daemon...");
		restart_daemon(&client, &daemon_path, data_dir, instance.as_deref()).await?;
	}

	println!();
	println!("Successfully updated to {}", available);

	Ok(AttemptResult::Updated)
}

/// `sd update status`: the running build, the marker beside the binaries,
/// the last attempt and any download waiting to resume.
pub fn status(data_dir: &Path, instance: Option<&str>) -> Result<()> {
	let config = CliConfig::load(&data_dir.to_path_buf())?;
	println!("Running binary: {}", VERSION_STRING);

	let current_exe = std::env::current_exe()?;
	let current_exe = current_exe.canonicalize().unwrap_or(current_exe);
	if let Some(bin_dir) = current_exe.parent() {
		let marker = std::fs::read_to_string(bin_dir.join(VERSION_FILE))
			.map(|s| s.trim().to_string())
			.unwrap_or_else(|_| "not written".to_string());
		println!("Installed marker ({}): {}", VERSION_FILE, marker);
	}
	println!(
		"Channel: {} ({})",
		config.update.channel, config.update.repo
	);

	match UpdateState::load(data_dir) {
		Some(state) => {
			let age = Utc::now().signed_duration_since(state.last_attempt);
			println!(
				"Last attempt: {} ({} ago) on {}",
				state
					.last_attempt
					.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
				format_age(age),
				state.channel
			);
			println!("Result: {}", state.summary());
		}
		None => println!("Last attempt: none recorded"),
	}

	let part_dir = data_dir.join("updates");
	if let Ok(entries) = std::fs::read_dir(&part_dir) {
		for entry in entries.flatten() {
			let name = entry.file_name().to_string_lossy().into_owned();
			if !name.ends_with(".part") {
				continue;
			}
			let len = entry.metadata().map(|m| m.len()).unwrap_or(0);
			println!("Resumable download: {} ({} bytes so far)", name, len);
		}
	}
	println!(
		"Log: {}",
		data_dir.join("logs").join("update.log").display()
	);
	println!();
	timer::print_status(instance)
}

fn format_age(age: chrono::Duration) -> String {
	let secs = age.num_seconds().max(0);
	if secs < 60 {
		format!("{}s", secs)
	} else if secs < 3600 {
		format!("{}m", secs / 60)
	} else if secs < 86_400 {
		format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
	} else {
		format!("{}d {}h", secs / 86_400, (secs % 86_400) / 3600)
	}
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
	let url = format!("{}/repos/{}/releases/{}", GITHUB_API, repo, selector);
	fetch_json(&url)
		.await
		.with_context(|| format!("fetching release {} from {}", selector, repo))
}

/// The commit the `nightly` tag points at.
///
/// The tag ref is the only thing the workflow moves on every push; the
/// release's `target_commitish` stays at whatever it was when the release was
/// created. An annotated tag is dereferenced through the tags API. The ref API
/// shares the unauthenticated rate limit with the release lookup, so when it
/// fails the `nightly.sha` asset, served from the release's download host, is
/// read instead.
async fn resolve_nightly_sha(
	api_base: &str,
	repo: &str,
	release: &GitHubRelease,
) -> Result<String> {
	let from_ref = async {
		let url = format!("{}/repos/{}/git/ref/tags/{}", api_base, repo, NIGHTLY_TAG);
		let mut object = fetch_json::<GitHubRefObject>(&url).await?.object;
		if object.kind == "tag" {
			let url = format!("{}/repos/{}/git/tags/{}", api_base, repo, object.sha);
			object = fetch_json::<GitHubRefObject>(&url).await?.object;
		}
		if object.kind != "commit" || !looks_like_sha(&object.sha) {
			return Err(anyhow::anyhow!(
				"tag {} points at {} {}, not a commit",
				NIGHTLY_TAG,
				object.kind,
				object.sha
			));
		}
		Ok::<_, anyhow::Error>(object.sha.to_ascii_lowercase())
	};

	let ref_error = match from_ref.await {
		Ok(sha) => return Ok(sha),
		Err(e) => e,
	};
	println!(
		"  git ref lookup failed ({:#}); reading {} from the release",
		ref_error, NIGHTLY_SHA_ASSET
	);

	let asset = release
		.assets
		.iter()
		.find(|a| a.name == NIGHTLY_SHA_ASSET)
		.ok_or_else(|| {
			anyhow::anyhow!(
				"Nightly release '{}' has no {} asset to fall back on",
				release.tag_name,
				NIGHTLY_SHA_ASSET
			)
		})?;
	let text = download_small(&asset.browser_download_url)
		.await
		.with_context(|| format!("downloading {}", NIGHTLY_SHA_ASSET))?;
	let sha = String::from_utf8_lossy(&text).trim().to_ascii_lowercase();
	if !looks_like_sha(&sha) {
		return Err(anyhow::anyhow!(
			"{} does not contain a commit sha: '{}'",
			NIGHTLY_SHA_ASSET,
			sha
		));
	}
	Ok(sha)
}

async fn fetch_json<T: serde::de::DeserializeOwned>(url: &str) -> Result<T> {
	let response = tokio::time::timeout(DOWNLOAD_IDLE_TIMEOUT, http_client()?.get(url).send())
		.await
		.map_err(|_| anyhow::anyhow!("timed out reaching {}", url))??;

	if !response.status().is_success() {
		return Err(anyhow::anyhow!("{}: HTTP {}", url, response.status()));
	}

	tokio::time::timeout(DOWNLOAD_IDLE_TIMEOUT, response.json())
		.await
		.map_err(|_| anyhow::anyhow!("timed out reading {}", url))?
		.with_context(|| format!("decoding {}", url))
}

/// Downloads an asset into a resumable part file and checks it against its
/// `.sha256` sibling when the release ships one. Only a verified part is
/// read back; a mismatch discards the part so the next tick starts clean.
async fn download_verified(
	assets: &[GitHubAsset],
	asset: &GitHubAsset,
	part_dir: &Path,
	build_key: &str,
) -> Result<Vec<u8>> {
	let checksum_name = format!("{}.sha256", asset.name);
	let expected = match assets.iter().find(|a| a.name == checksum_name) {
		Some(checksum_asset) => {
			let text = download_small(&checksum_asset.browser_download_url)
				.await
				.with_context(|| format!("downloading {}", checksum_name))?;
			Some(
				parse_sha256(&String::from_utf8_lossy(&text)).ok_or_else(|| {
					anyhow::anyhow!("{} does not contain a sha256 digest", checksum_name)
				})?,
			)
		}
		None => {
			println!(
				"  no {} published, skipping checksum verification",
				checksum_name
			);
			None
		}
	};

	std::fs::create_dir_all(part_dir)?;
	let part = part_dir.join(format!("{}.{}.part", asset.name, build_key));
	download_resumable(&asset.browser_download_url, asset.size, &part)
		.await
		.with_context(|| format!("downloading {}", asset.name))?;

	let data = std::fs::read(&part).with_context(|| format!("reading {}", part.display()))?;
	if let Some(expected) = expected {
		let actual = Sha256::digest(&data)
			.iter()
			.map(|b| format!("{:02x}", b))
			.collect::<String>();
		if actual != expected {
			let _ = std::fs::remove_file(&part);
			return Err(anyhow::anyhow!(
				"Checksum mismatch for {}: expected {}, got {}; discarded the download",
				asset.name,
				expected,
				actual
			));
		}
		println!("  sha256 verified");
	}
	Ok(data)
}

fn http_client() -> Result<reqwest::Client> {
	Ok(reqwest::Client::builder()
		.user_agent("spacedrive-cli")
		.connect_timeout(Duration::from_secs(30))
		.build()?)
}

/// Fetches a small asset (a checksum file, a release document) whole, with
/// the same idle bound on the body as on the connection so a stalled proxy
/// cannot hang a timer run.
async fn download_small(url: &str) -> Result<Vec<u8>> {
	let response = tokio::time::timeout(DOWNLOAD_IDLE_TIMEOUT, http_client()?.get(url).send())
		.await
		.map_err(|_| anyhow::anyhow!("timed out connecting"))??;
	if !response.status().is_success() {
		return Err(anyhow::anyhow!(
			"Failed to download: HTTP {}",
			response.status()
		));
	}
	Ok(
		tokio::time::timeout(DOWNLOAD_IDLE_TIMEOUT, response.bytes())
			.await
			.map_err(|_| anyhow::anyhow!("timed out reading the response"))??
			.to_vec(),
	)
}

/// Fetches `url` into `part`, continuing from whatever an earlier attempt
/// left there. A server that ignores the `Range` header answers 200 and the
/// part is rewritten from the start. The part survives every failure except
/// a size overrun, so a dropped link costs only the bytes not yet received.
async fn download_resumable(url: &str, expected_size: u64, part: &Path) -> Result<()> {
	let mut have = std::fs::metadata(part).map(|m| m.len()).unwrap_or(0);
	if have > expected_size {
		let _ = std::fs::remove_file(part);
		have = 0;
	}
	if have == expected_size && have > 0 {
		println!("  already downloaded, verifying");
		return Ok(());
	}
	if have > 0 {
		println!("  resuming from {} of {} bytes", have, expected_size);
	}

	let mut request = http_client()?.get(url);
	if have > 0 {
		request = request.header(reqwest::header::RANGE, format!("bytes={}-", have));
	}
	let mut response = tokio::time::timeout(DOWNLOAD_IDLE_TIMEOUT, request.send())
		.await
		.map_err(|_| anyhow::anyhow!("timed out connecting"))??;

	let mut file = match response.status() {
		reqwest::StatusCode::PARTIAL_CONTENT if have > 0 => {
			std::fs::OpenOptions::new().append(true).open(part)?
		}
		status if status.is_success() => {
			have = 0;
			std::fs::File::create(part)?
		}
		status => {
			return Err(anyhow::anyhow!("Failed to download: HTTP {}", status));
		}
	};

	loop {
		let chunk = tokio::time::timeout(DOWNLOAD_IDLE_TIMEOUT, response.chunk())
			.await
			.map_err(|_| {
				anyhow::anyhow!(
					"no data for {}s after {} of {} bytes; will resume next attempt",
					DOWNLOAD_IDLE_TIMEOUT.as_secs(),
					have,
					expected_size
				)
			})?
			.with_context(|| {
				format!(
					"connection dropped after {} of {} bytes; will resume next attempt",
					have, expected_size
				)
			})?;
		let Some(chunk) = chunk else { break };
		file.write_all(&chunk)?;
		have += chunk.len() as u64;
		if have > expected_size {
			drop(file);
			let _ = std::fs::remove_file(part);
			return Err(anyhow::anyhow!(
				"Downloaded more than the published size ({} > {}); discarded",
				have,
				expected_size
			));
		}
	}
	file.flush()?;

	if have != expected_size {
		return Err(anyhow::anyhow!(
			"Download ended after {} of {} bytes; will resume next attempt",
			have,
			expected_size
		));
	}
	Ok(())
}

/// Removes parts left by a different build so they cannot be resumed into.
fn discard_other_parts(part_dir: &Path, build_key: &str) {
	let Ok(entries) = std::fs::read_dir(part_dir) else {
		return;
	};
	let keep = format!(".{}.part", build_key);
	for entry in entries.flatten() {
		let name = entry.file_name().to_string_lossy().into_owned();
		if name.ends_with(".part") && !name.ends_with(&keep) {
			let _ = std::fs::remove_file(entry.path());
		}
	}
}

/// Replaces `path` with `data` without ever truncating the running binary.
///
/// The new bytes land in a sibling `.update` file, the current binary is
/// renamed to `.bak`, and the staged file is renamed into place. Moving the
/// running binary aside instead of renaming over it is what Windows allows: a
/// loaded executable can be renamed but not replaced or deleted, so the
/// `.bak` of the running `sd` stays behind there until the next update
/// removes it. A failed install renames the backup back.
fn replace_binary(path: &Path, data: &[u8]) -> Result<()> {
	use std::fs;

	let file_name = path
		.file_name()
		.and_then(|n| n.to_str())
		.ok_or_else(|| anyhow::anyhow!("Invalid binary path: {}", path.display()))?;
	let staged = path.with_file_name(format!("{}.update", file_name));
	let backup = path.with_file_name(format!("{}.bak", file_name));
	let _ = fs::remove_file(&backup);

	fs::write(&staged, data).with_context(|| format!("writing {}", staged.display()))?;

	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt;
		fs::set_permissions(&staged, fs::Permissions::from_mode(0o755))?;
	}

	let had_previous = path.exists();
	if had_previous {
		fs::rename(path, &backup).with_context(|| format!("moving {} aside", path.display()))?;
	}

	match fs::rename(&staged, path) {
		Ok(()) => {
			let _ = fs::remove_file(&backup);
			Ok(())
		}
		Err(e) => {
			if had_previous {
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

/// True when `candidate` is a strictly newer semantic version than
/// `current`. The repository's latest release can predate the installed
/// build (the v1 desktop releases live under the same repository), so a
/// mismatch alone is not an update. Tags that do not parse as semver are
/// treated as newer so a renamed scheme still reaches the user.
fn is_newer_version(candidate: &str, current: &str) -> bool {
	match (
		semver::Version::parse(candidate),
		semver::Version::parse(current),
	) {
		(Ok(candidate), Ok(current)) => candidate > current,
		_ => candidate != current,
	}
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
	fn replace_binary_swaps_the_file_and_leaves_no_backup() {
		let dir = tempfile::tempdir().unwrap();
		let name = format!("sd{}", std::env::consts::EXE_SUFFIX);
		let path = dir.path().join(&name);
		std::fs::write(&path, b"old").unwrap();
		std::fs::write(dir.path().join(format!("{name}.bak")), b"stale").unwrap();

		replace_binary(&path, b"new").unwrap();

		assert_eq!(std::fs::read(&path).unwrap(), b"new");
		let leftovers: Vec<_> = std::fs::read_dir(dir.path())
			.unwrap()
			.map(|e| e.unwrap().file_name())
			.collect();
		assert_eq!(leftovers, vec![std::ffi::OsString::from(name)]);
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
	fn older_release_is_not_an_update() {
		assert!(!is_newer_version("0.4.3", "2.0.0-alpha.2"));
		assert!(!is_newer_version("2.0.0-alpha.2", "2.0.0-alpha.2"));
		assert!(is_newer_version("2.0.0-beta.1", "2.0.0-alpha.2"));
		assert!(is_newer_version("2.0.0", "2.0.0-beta.1"));
		assert!(is_newer_version("weekly-12", "2.0.0-alpha.2"));
	}

	/// Serves fixed responses by path so the sha resolution can be exercised
	/// without GitHub. Returns the base URL.
	async fn serve(routes: Vec<(&'static str, u16, &'static str)>) -> String {
		use tokio::io::{AsyncReadExt, AsyncWriteExt};

		let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
		let base = format!("http://{}", listener.local_addr().unwrap());
		tokio::spawn(async move {
			loop {
				let Ok((mut stream, _)) = listener.accept().await else {
					return;
				};
				let routes = routes.clone();
				tokio::spawn(async move {
					let mut buf = vec![0u8; 4096];
					let n = stream.read(&mut buf).await.unwrap_or(0);
					let request = String::from_utf8_lossy(&buf[..n]).into_owned();
					let path = request.split_whitespace().nth(1).unwrap_or("/").to_string();
					let (status, body) = routes
						.iter()
						.find(|(p, _, _)| *p == path)
						.map(|(_, status, body)| (*status, *body))
						.unwrap_or((404, "not found"));
					let response = format!(
						"HTTP/1.1 {} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
						status,
						body.len(),
						body
					);
					let _ = stream.write_all(response.as_bytes()).await;
					let _ = stream.shutdown().await;
				});
			}
		});
		base
	}

	const REF_SHA: &str = "0e5340394e1a233fb4f7f5977fd637e9f2493c04";
	const ASSET_SHA: &str = "ad6452d6f1c7b2d9a5a1e0c3b4d5e6f708192a3b";

	fn nightly_release(base: &str) -> GitHubRelease {
		GitHubRelease {
			tag_name: "nightly".to_string(),
			assets: vec![
				asset("sd-macos-aarch64"),
				GitHubAsset {
					name: NIGHTLY_SHA_ASSET.to_string(),
					browser_download_url: format!("{}/download/nightly/nightly.sha", base),
					size: 41,
				},
			],
		}
	}

	#[tokio::test]
	async fn nightly_sha_comes_from_the_tag_ref() {
		let base = serve(vec![
			(
				"/repos/o/r/git/ref/tags/nightly",
				200,
				r#"{"ref":"refs/tags/nightly","object":{"sha":"0E5340394E1A233FB4F7F5977FD637E9F2493C04","type":"commit"}}"#,
			),
			("/download/nightly/nightly.sha", 200, ASSET_SHA),
		])
		.await;

		let sha = resolve_nightly_sha(&base, "o/r", &nightly_release(&base))
			.await
			.unwrap();
		assert_eq!(sha, REF_SHA);
	}

	#[tokio::test]
	async fn annotated_tag_is_dereferenced() {
		let base = serve(vec![
			(
				"/repos/o/r/git/ref/tags/nightly",
				200,
				r#"{"object":{"sha":"1111111111111111111111111111111111111111","type":"tag"}}"#,
			),
			(
				"/repos/o/r/git/tags/1111111111111111111111111111111111111111",
				200,
				r#"{"tag":"nightly","object":{"sha":"0e5340394e1a233fb4f7f5977fd637e9f2493c04","type":"commit"}}"#,
			),
		])
		.await;

		let sha = resolve_nightly_sha(&base, "o/r", &nightly_release(&base))
			.await
			.unwrap();
		assert_eq!(sha, REF_SHA);
	}

	#[tokio::test]
	async fn rate_limited_ref_api_falls_back_to_the_sha_asset() {
		let base = serve(vec![
			(
				"/repos/o/r/git/ref/tags/nightly",
				403,
				r#"{"message":"API rate limit exceeded"}"#,
			),
			(
				"/download/nightly/nightly.sha",
				200,
				"ad6452d6f1c7b2d9a5a1e0c3b4d5e6f708192a3b\n",
			),
		])
		.await;

		let sha = resolve_nightly_sha(&base, "o/r", &nightly_release(&base))
			.await
			.unwrap();
		assert_eq!(sha, ASSET_SHA);
	}

	#[tokio::test]
	async fn missing_ref_and_asset_is_an_error() {
		let base = serve(vec![("/download/nightly/nightly.sha", 200, "not a sha")]).await;

		let err = resolve_nightly_sha(&base, "o/r", &nightly_release(&base))
			.await
			.unwrap_err();
		assert!(
			err.to_string().contains("does not contain a commit sha"),
			"{err:#}"
		);

		let mut release = nightly_release(&base);
		release.assets.retain(|a| a.name != NIGHTLY_SHA_ASSET);
		let err = resolve_nightly_sha(&base, "o/r", &release)
			.await
			.unwrap_err();
		assert!(
			err.to_string().contains("has no nightly.sha asset"),
			"{err:#}"
		);
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
