//! Machine-scoped discovery and installation of optional host tools.
//!
//! Spacedrive keeps large native dependencies out of its default bundle. This
//! service turns software the person installed on the host into an explicit,
//! typed capability. Discovery includes well-known package-manager locations
//! because desktop applications inherit a smaller `PATH` than interactive
//! shells on macOS and Windows.
//!
//! Tool execution never passes through a shell. Every caller selects a
//! registered tool and supplies arguments to its resolved executable. Install
//! requests are similarly limited to fixed package-manager recipes and require
//! an explicit confirmation bit in the action input.

use std::{
	ffi::OsString,
	fs,
	io::{Read, Seek, SeekFrom},
	path::{Path, PathBuf},
	process::{Command, ExitStatus, Stdio},
	sync::RwLock,
	thread,
	time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use specta::Type;
use tempfile::tempfile;
use thiserror::Error;
use tokio::process::Command as TokioCommand;
use tracing::info;

const FFMPEG_PATH_ENV: &str = "SPACEDRIVE_FFMPEG_PATH";
const FFPROBE_PATH_ENV: &str = "SPACEDRIVE_FFPROBE_PATH";
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
const PROBE_OUTPUT_LIMIT: usize = 1024 * 1024;

/// A dependency Spacedrive knows how to discover and use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ExternalToolId {
	Ffmpeg,
}

impl ExternalToolId {
	pub const fn as_str(self) -> &'static str {
		match self {
			Self::Ffmpeg => "ffmpeg",
		}
	}
}

/// A feature made runnable by an external tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ToolCapability {
	VideoThumbnail,
	VideoThumbstrip,
	MediaMetadata,
}

/// How an executable was found on this machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ToolSource {
	Environment,
	Path,
	KnownLocation,
}

/// A package manager with a reviewed install recipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ToolInstaller {
	Homebrew,
	Winget,
}

impl ToolInstaller {
	pub const fn as_str(self) -> &'static str {
		match self {
			Self::Homebrew => "homebrew",
			Self::Winget => "winget",
		}
	}
}

/// One way the current host can install a missing tool.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ToolInstallOption {
	pub installer: ToolInstaller,
	pub label: String,
	pub available: bool,
}

/// Current machine-local state for an optional tool.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ExternalToolStatus {
	pub id: ExternalToolId,
	pub name: String,
	pub available: bool,
	pub path: Option<PathBuf>,
	pub companion_paths: Vec<PathBuf>,
	pub version: Option<String>,
	pub source: Option<ToolSource>,
	pub capabilities: Vec<ToolCapability>,
	pub install_options: Vec<ToolInstallOption>,
}

#[derive(Debug, Clone)]
struct ResolvedExecutable {
	path: PathBuf,
	source: ToolSource,
}

#[derive(Debug, Default)]
struct ToolState {
	ffmpeg: Option<ResolvedExecutable>,
	ffprobe: Option<ResolvedExecutable>,
}

/// Captured output from one registered tool invocation.
#[derive(Debug)]
pub struct ToolOutput {
	pub status: ExitStatus,
	pub stdout: Vec<u8>,
	pub stderr: String,
}

#[derive(Debug, Error)]
pub enum ToolError {
	#[error("{0} is not installed")]
	Unavailable(&'static str),
	#[error("could not start {tool}: {source}")]
	Spawn {
		tool: &'static str,
		#[source]
		source: std::io::Error,
	},
	#[error("{tool} exceeded its {seconds} second execution limit")]
	TimedOut { tool: &'static str, seconds: u64 },
	#[error("could not capture {tool} output: {source}")]
	Output {
		tool: &'static str,
		#[source]
		source: std::io::Error,
	},
	#[error("{tool} produced more than {limit} bytes of output")]
	OutputTooLarge { tool: &'static str, limit: usize },
	#[error("{0} is not available on this machine")]
	InstallerUnavailable(&'static str),
	#[error("{installer} could not install {tool}: {message}")]
	InstallFailed {
		installer: &'static str,
		tool: &'static str,
		message: String,
	},
	#[error("automatic installation with {0} is not supported on this platform")]
	UnsupportedInstaller(&'static str),
}

/// Registry for optional software installed on the host.
pub struct ExternalTools {
	state: RwLock<ToolState>,
}

impl ExternalTools {
	pub fn new() -> Self {
		Self {
			state: RwLock::new(discover()),
		}
	}

	/// Resolve FFmpeg, refreshing a previous miss so software installed while
	/// the daemon is running becomes usable without a restart.
	pub fn ffmpeg_path(&self) -> Option<PathBuf> {
		self.resolve_or_refresh(|state| state.ffmpeg.as_ref().map(|entry| entry.path.clone()))
	}

	/// Resolve FFprobe from the same package. Thumbstrips require duration
	/// evidence so their frames cover the whole video rather than an arbitrary
	/// prefix.
	pub fn ffprobe_path(&self) -> Option<PathBuf> {
		self.resolve_or_refresh(|state| state.ffprobe.as_ref().map(|entry| entry.path.clone()))
	}

	fn resolve_or_refresh(
		&self,
		select: impl Fn(&ToolState) -> Option<PathBuf>,
	) -> Option<PathBuf> {
		if let Some(path) = {
			let state = self.state.read().unwrap_or_else(|e| e.into_inner());
			select(&state)
		} {
			if is_executable(&path) {
				return Some(path);
			}
		}

		self.refresh();
		let state = self.state.read().unwrap_or_else(|e| e.into_inner());
		select(&state)
	}

	pub fn refresh(&self) {
		*self.state.write().unwrap_or_else(|e| e.into_inner()) = discover();
	}

	/// Return fresh status, including a bounded version probe.
	pub async fn statuses(self: &std::sync::Arc<Self>) -> Vec<ExternalToolStatus> {
		let tools = self.clone();
		let fallback = self.unavailable_ffmpeg_status();
		tokio::task::spawn_blocking(move || tools.statuses_blocking())
			.await
			.unwrap_or_else(|_| vec![fallback])
	}

	fn statuses_blocking(&self) -> Vec<ExternalToolStatus> {
		self.refresh();
		let state = self.state.read().unwrap_or_else(|e| e.into_inner());
		let version = state.ffmpeg.as_ref().and_then(|entry| {
			run_path(
				"ffmpeg",
				&entry.path,
				&[OsString::from("-version")],
				PROBE_TIMEOUT,
				PROBE_OUTPUT_LIMIT,
			)
			.ok()
			.filter(|output| output.status.success())
			.and_then(|output| first_line(&output.stdout))
		});

		vec![ExternalToolStatus {
			id: ExternalToolId::Ffmpeg,
			name: "FFmpeg".to_string(),
			available: state.ffmpeg.is_some(),
			path: state.ffmpeg.as_ref().map(|entry| entry.path.clone()),
			companion_paths: state
				.ffprobe
				.as_ref()
				.map(|entry| vec![entry.path.clone()])
				.unwrap_or_default(),
			version,
			source: state.ffmpeg.as_ref().map(|entry| entry.source),
			capabilities: ffmpeg_capabilities(&state),
			install_options: install_options(),
		}]
	}

	fn unavailable_ffmpeg_status(&self) -> ExternalToolStatus {
		ExternalToolStatus {
			id: ExternalToolId::Ffmpeg,
			name: "FFmpeg".to_string(),
			available: false,
			path: None,
			companion_paths: Vec::new(),
			version: None,
			source: None,
			capabilities: Vec::new(),
			install_options: install_options(),
		}
	}

	/// Run FFmpeg with fixed, caller-supplied arguments. No shell is involved.
	pub fn run_ffmpeg(
		&self,
		args: &[OsString],
		timeout: Duration,
		output_limit: usize,
	) -> Result<ToolOutput, ToolError> {
		let path = self.ffmpeg_path().ok_or(ToolError::Unavailable("ffmpeg"))?;
		run_path("ffmpeg", &path, args, timeout, output_limit)
	}

	/// Run FFprobe from the resolved FFmpeg package.
	pub fn run_ffprobe(
		&self,
		args: &[OsString],
		timeout: Duration,
		output_limit: usize,
	) -> Result<ToolOutput, ToolError> {
		let path = self
			.ffprobe_path()
			.ok_or(ToolError::Unavailable("ffprobe"))?;
		run_path("ffprobe", &path, args, timeout, output_limit)
	}

	/// Install one registered tool through a reviewed package-manager recipe.
	pub async fn install(
		&self,
		tool: ExternalToolId,
		installer: ToolInstaller,
	) -> Result<(), ToolError> {
		let (program, args) = install_command(tool, installer)?;
		info!(
			tool = tool.as_str(),
			installer = installer.as_str(),
			"Installing external tool"
		);
		let output = TokioCommand::new(&program)
			.args(args)
			.kill_on_drop(true)
			.output()
			.await
			.map_err(|_| ToolError::InstallerUnavailable(installer.as_str()))?;
		if !output.status.success() {
			let message = first_nonempty_line(&output.stderr)
				.or_else(|| first_nonempty_line(&output.stdout))
				.unwrap_or_else(|| format!("exit status {}", output.status));
			return Err(ToolError::InstallFailed {
				installer: installer.as_str(),
				tool: tool.as_str(),
				message,
			});
		}
		self.refresh();
		if self.ffmpeg_path().is_none() {
			return Err(ToolError::InstallFailed {
				installer: installer.as_str(),
				tool: tool.as_str(),
				message: "installation completed but the executable could not be discovered".into(),
			});
		}
		Ok(())
	}
}

impl Default for ExternalTools {
	fn default() -> Self {
		Self::new()
	}
}

fn discover() -> ToolState {
	let ffmpeg = discover_executable("ffmpeg", FFMPEG_PATH_ENV, &known_executable_paths("ffmpeg"));
	let sibling = ffmpeg
		.as_ref()
		.and_then(|entry| entry.path.parent())
		.map(|parent| parent.join(executable_name("ffprobe")));
	let ffprobe = discover_companion(
		"ffprobe",
		FFPROBE_PATH_ENV,
		sibling.as_deref(),
		&known_executable_paths("ffprobe"),
	);
	ToolState { ffmpeg, ffprobe }
}

fn discover_companion(
	name: &str,
	override_env: &str,
	preferred: Option<&Path>,
	known_paths: &[PathBuf],
) -> Option<ResolvedExecutable> {
	if let Some(path) = std::env::var_os(override_env).map(PathBuf::from) {
		if is_executable(&path) {
			return Some(ResolvedExecutable {
				path: canonical_or_original(path),
				source: ToolSource::Environment,
			});
		}
	}

	if let Some(path) = preferred.filter(|path| is_executable(path)) {
		return Some(ResolvedExecutable {
			path: canonical_or_original(path.to_path_buf()),
			source: ToolSource::KnownLocation,
		});
	}

	discover_executable(name, override_env, known_paths)
}

fn discover_executable(
	name: &str,
	override_env: &str,
	known_paths: &[PathBuf],
) -> Option<ResolvedExecutable> {
	if let Some(path) = std::env::var_os(override_env).map(PathBuf::from) {
		if is_executable(&path) {
			return Some(ResolvedExecutable {
				path: canonical_or_original(path),
				source: ToolSource::Environment,
			});
		}
	}

	if let Some(path) = find_on_path(name) {
		return Some(ResolvedExecutable {
			path: canonical_or_original(path),
			source: ToolSource::Path,
		});
	}

	known_paths.iter().find_map(|path| {
		is_executable(path).then(|| ResolvedExecutable {
			path: canonical_or_original(path.clone()),
			source: ToolSource::KnownLocation,
		})
	})
}

fn find_on_path(name: &str) -> Option<PathBuf> {
	let path = std::env::var_os("PATH")?;
	let names = executable_names(name);
	std::env::split_paths(&path).find_map(|dir| {
		names.iter().find_map(|name| {
			let candidate = dir.join(name);
			is_executable(&candidate).then_some(candidate)
		})
	})
}

fn known_executable_paths(name: &str) -> Vec<PathBuf> {
	let executable = executable_name(name);
	let mut paths = Vec::new();

	#[cfg(target_os = "macos")]
	for prefix in [
		"/opt/homebrew/bin",
		"/usr/local/bin",
		"/opt/local/bin",
		"/usr/bin",
	] {
		paths.push(Path::new(prefix).join(&executable));
	}

	#[cfg(target_os = "linux")]
	for prefix in [
		"/usr/local/bin",
		"/usr/bin",
		"/snap/bin",
		"/var/lib/flatpak/exports/bin",
	] {
		paths.push(Path::new(prefix).join(&executable));
	}

	#[cfg(target_os = "windows")]
	{
		if let Some(local) = std::env::var_os("LOCALAPPDATA") {
			paths.push(
				PathBuf::from(local)
					.join("Microsoft")
					.join("WinGet")
					.join("Links")
					.join(&executable),
			);
		}
		for variable in ["ProgramFiles", "ProgramFiles(x86)"] {
			if let Some(root) = std::env::var_os(variable) {
				paths.push(
					PathBuf::from(root)
						.join("ffmpeg")
						.join("bin")
						.join(&executable),
				);
			}
		}
	}

	paths
}

fn executable_name(name: &str) -> OsString {
	#[cfg(target_os = "windows")]
	{
		return format!("{name}.exe").into();
	}
	#[cfg(not(target_os = "windows"))]
	{
		name.into()
	}
}

fn executable_names(name: &str) -> Vec<OsString> {
	#[cfg(target_os = "windows")]
	{
		let mut names = vec![format!("{name}.exe").into(), name.into()];
		if let Some(extensions) = std::env::var_os("PATHEXT") {
			for extension in extensions.to_string_lossy().split(';') {
				if !extension.is_empty() {
					names.push(format!("{name}{extension}").into());
				}
			}
		}
		return names;
	}
	#[cfg(not(target_os = "windows"))]
	{
		vec![name.into()]
	}
}

fn is_executable(path: &Path) -> bool {
	let Ok(metadata) = fs::metadata(path) else {
		return false;
	};
	if !metadata.is_file() {
		return false;
	}
	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt;
		metadata.permissions().mode() & 0o111 != 0
	}
	#[cfg(not(unix))]
	{
		true
	}
}

fn canonical_or_original(path: PathBuf) -> PathBuf {
	path.canonicalize().unwrap_or(path)
}

fn ffmpeg_capabilities(state: &ToolState) -> Vec<ToolCapability> {
	if state.ffmpeg.is_none() {
		return Vec::new();
	}
	let mut capabilities = vec![ToolCapability::VideoThumbnail];
	if state.ffprobe.is_some() {
		capabilities.push(ToolCapability::MediaMetadata);
		capabilities.push(ToolCapability::VideoThumbstrip);
	}
	capabilities
}

fn install_options() -> Vec<ToolInstallOption> {
	vec![
		ToolInstallOption {
			installer: ToolInstaller::Homebrew,
			label: "Install with Homebrew".to_string(),
			available: cfg!(target_os = "macos") && package_manager_path("brew").is_some(),
		},
		ToolInstallOption {
			installer: ToolInstaller::Winget,
			label: "Install with WinGet".to_string(),
			available: cfg!(target_os = "windows") && package_manager_path("winget").is_some(),
		},
	]
}

fn install_command(
	tool: ExternalToolId,
	installer: ToolInstaller,
) -> Result<(PathBuf, Vec<OsString>), ToolError> {
	match (tool, installer) {
		(ExternalToolId::Ffmpeg, ToolInstaller::Homebrew) if cfg!(target_os = "macos") => {
			let program =
				package_manager_path("brew").ok_or(ToolError::InstallerUnavailable("homebrew"))?;
			Ok((program, vec!["install".into(), "ffmpeg".into()]))
		}
		(ExternalToolId::Ffmpeg, ToolInstaller::Winget) if cfg!(target_os = "windows") => {
			let program =
				package_manager_path("winget").ok_or(ToolError::InstallerUnavailable("winget"))?;
			Ok((
				program,
				vec![
					"install".into(),
					"--id".into(),
					"Gyan.FFmpeg".into(),
					"--exact".into(),
					"--accept-source-agreements".into(),
					"--accept-package-agreements".into(),
				],
			))
		}
		(_, installer) => Err(ToolError::UnsupportedInstaller(installer.as_str())),
	}
}

fn package_manager_path(name: &str) -> Option<PathBuf> {
	if let Some(path) = find_on_path(name) {
		return Some(path);
	}
	#[cfg(target_os = "windows")]
	if name == "winget" {
		if let Some(local) = std::env::var_os("LOCALAPPDATA") {
			let path = PathBuf::from(local)
				.join("Microsoft")
				.join("WindowsApps")
				.join("winget.exe");
			if is_executable(&path) {
				return Some(path);
			}
		}
	}
	#[cfg(target_os = "macos")]
	if name == "brew" {
		return ["/opt/homebrew/bin/brew", "/usr/local/bin/brew"]
			.into_iter()
			.map(PathBuf::from)
			.find(|path| is_executable(path));
	}
	None
}

fn run_path(
	tool: &'static str,
	path: &Path,
	args: &[OsString],
	timeout: Duration,
	output_limit: usize,
) -> Result<ToolOutput, ToolError> {
	let mut stdout = tempfile().map_err(|source| ToolError::Output { tool, source })?;
	let mut stderr = tempfile().map_err(|source| ToolError::Output { tool, source })?;
	let child_stdout = stdout
		.try_clone()
		.map(Stdio::from)
		.map_err(|source| ToolError::Output { tool, source })?;
	let child_stderr = stderr
		.try_clone()
		.map(Stdio::from)
		.map_err(|source| ToolError::Output { tool, source })?;

	let mut child = Command::new(path)
		.args(args)
		.stdin(Stdio::null())
		.stdout(child_stdout)
		.stderr(child_stderr)
		.spawn()
		.map_err(|source| ToolError::Spawn { tool, source })?;

	let started = Instant::now();
	let status = loop {
		if output_exceeds_limit(tool, &stdout, &stderr, output_limit)? {
			let _ = child.kill();
			let _ = child.wait();
			return Err(ToolError::OutputTooLarge {
				tool,
				limit: output_limit,
			});
		}
		match child
			.try_wait()
			.map_err(|source| ToolError::Output { tool, source })?
		{
			Some(status) => break status,
			None if started.elapsed() >= timeout => {
				let _ = child.kill();
				let _ = child.wait();
				return Err(ToolError::TimedOut {
					tool,
					seconds: timeout.as_secs(),
				});
			}
			None => thread::sleep(Duration::from_millis(20)),
		}
	};

	let stdout = read_limited(tool, &mut stdout, output_limit)?;
	let stderr = read_limited(tool, &mut stderr, output_limit)?;
	Ok(ToolOutput {
		status,
		stdout,
		stderr: String::from_utf8_lossy(&stderr).into_owned(),
	})
}

fn output_exceeds_limit(
	tool: &'static str,
	stdout: &fs::File,
	stderr: &fs::File,
	limit: usize,
) -> Result<bool, ToolError> {
	for file in [stdout, stderr] {
		let len = file
			.metadata()
			.map_err(|source| ToolError::Output { tool, source })?
			.len();
		if len > limit as u64 {
			return Ok(true);
		}
	}
	Ok(false)
}

fn read_limited(
	tool: &'static str,
	file: &mut fs::File,
	limit: usize,
) -> Result<Vec<u8>, ToolError> {
	let len = file
		.metadata()
		.map_err(|source| ToolError::Output { tool, source })?
		.len() as usize;
	if len > limit {
		return Err(ToolError::OutputTooLarge { tool, limit });
	}
	file.seek(SeekFrom::Start(0))
		.map_err(|source| ToolError::Output { tool, source })?;
	let mut output = Vec::with_capacity(len);
	file.read_to_end(&mut output)
		.map_err(|source| ToolError::Output { tool, source })?;
	Ok(output)
}

fn first_line(bytes: &[u8]) -> Option<String> {
	first_nonempty_line(bytes)
}

fn first_nonempty_line(bytes: &[u8]) -> Option<String> {
	String::from_utf8_lossy(bytes)
		.lines()
		.map(str::trim)
		.find(|line| !line.is_empty())
		.map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::collections::HashSet;

	#[test]
	fn version_probe_keeps_one_meaningful_line() {
		assert_eq!(
			first_line(b"ffmpeg version 8.0.1 Copyright\nconfiguration: large\n"),
			Some("ffmpeg version 8.0.1 Copyright".to_string())
		);
	}

	#[test]
	fn thumbstrips_require_the_ffprobe_companion() {
		let executable = ResolvedExecutable {
			path: PathBuf::from("ffmpeg"),
			source: ToolSource::Path,
		};
		let without_probe = ToolState {
			ffmpeg: Some(executable.clone()),
			ffprobe: None,
		};
		assert!(!ffmpeg_capabilities(&without_probe).contains(&ToolCapability::VideoThumbstrip));

		let with_probe = ToolState {
			ffmpeg: Some(executable.clone()),
			ffprobe: Some(executable),
		};
		assert!(ffmpeg_capabilities(&with_probe).contains(&ToolCapability::VideoThumbstrip));
	}

	#[test]
	fn bounded_runner_captures_stdout_without_a_shell() {
		#[cfg(unix)]
		{
			let output = run_path(
				"printf",
				Path::new("/usr/bin/printf"),
				&[OsString::from("hello")],
				Duration::from_secs(1),
				32,
			)
			.expect("run printf");
			assert!(output.status.success());
			assert_eq!(output.stdout, b"hello");
		}
	}

	#[test]
	fn candidate_paths_are_unique() {
		let paths = known_executable_paths("ffmpeg");
		let unique: HashSet<_> = paths.iter().collect();
		assert_eq!(unique.len(), paths.len());
	}

	#[tokio::test]
	async fn status_matches_discovery_and_probes_an_installed_binary() {
		let tools = std::sync::Arc::new(ExternalTools::new());
		let discovered = tools.ffmpeg_path();
		let statuses = tools.statuses().await;
		assert_eq!(statuses.len(), 1);
		let status = &statuses[0];
		assert_eq!(status.available, discovered.is_some());
		if discovered.is_some() {
			assert!(status
				.version
				.as_deref()
				.is_some_and(|line| line.starts_with("ffmpeg version")));
			assert!(status
				.capabilities
				.contains(&ToolCapability::VideoThumbnail));
		}
	}
}
