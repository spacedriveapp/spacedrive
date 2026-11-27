//! # Command line tool install
//!
//! The desktop app ships `sd-cli` beside `sd-daemon`, built from the same
//! commit with the same features, so a CLI linked from the app always matches
//! the daemon it talks to. These ops put `sd` on the PATH as a link to that
//! copy. The daemon answers them because the app always starts it from inside
//! the bundle, so the CLI beside the daemon's own executable is the app's.
//!
//! On macOS the link goes in `/usr/local/bin`, which every shell has on its
//! default PATH. When that folder is not writable the daemon asks for an
//! administrator through the standard macOS prompt, and when that is refused
//! it links `~/.local/bin/sd` instead. Linux always uses `~/.local/bin`.

pub mod action;
pub mod query;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use specta::Type;
use tracing::info;

pub use action::{CliInstallAction, CliUninstallAction};
pub use query::CliStatusQuery;

/// The link macOS installs, on every shell's default PATH.
#[cfg(target_os = "macos")]
const SYSTEM_LINK: &str = "/usr/local/bin/sd";

/// What is at the place `sd` resolves to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CliLink {
	/// Nothing at any location the install writes to.
	NotInstalled,
	/// `path` links to the CLI shipped with this daemon.
	Bundled { path: String },
	/// `path` is another binary or a link to one, such as a standalone
	/// download or a copy of the app somewhere else. `target` is where the
	/// link points, and is absent for a plain file.
	Other {
		path: String,
		target: Option<String>,
	},
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct CliInstallStatus {
	/// The CLI shipped beside this daemon, absent when this daemon has none,
	/// such as a standalone install.
	pub bundled: Option<String>,
	pub link: CliLink,
	/// The folder to add to PATH, set when the link is in `~/.local/bin`,
	/// which shells do not search by default.
	pub path_hint: Option<String>,
}

/// The CLI beside the running daemon, resolved through any links.
async fn bundled_cli() -> Option<PathBuf> {
	let exe = std::env::current_exe().ok()?;
	let exe = tokio::fs::canonicalize(&exe).await.unwrap_or(exe);
	let cli = exe
		.parent()?
		.join(format!("sd-cli{}", std::env::consts::EXE_SUFFIX));
	let cli = tokio::fs::canonicalize(cli).await.ok()?;
	let meta = tokio::fs::metadata(&cli).await.ok()?;
	meta.is_file().then_some(cli)
}

fn user_link() -> Option<PathBuf> {
	dirs::home_dir().map(|home| home.join(".local/bin/sd"))
}

/// Every place the install writes, in the order a default PATH searches them.
fn link_locations() -> Vec<PathBuf> {
	let mut locations = Vec::new();
	#[cfg(target_os = "macos")]
	locations.push(PathBuf::from(SYSTEM_LINK));
	locations.extend(user_link());
	locations
}

/// What is at `path`, or None when nothing is.
async fn link_at(path: &Path, bundled: Option<&Path>) -> Option<CliLink> {
	let meta = tokio::fs::symlink_metadata(path).await.ok()?;
	let display = path.display().to_string();
	if !meta.file_type().is_symlink() {
		return Some(CliLink::Other {
			path: display,
			target: None,
		});
	}
	let resolved = tokio::fs::canonicalize(path).await.ok();
	if resolved.is_some() && resolved.as_deref() == bundled {
		return Some(CliLink::Bundled { path: display });
	}
	let target = tokio::fs::read_link(path)
		.await
		.ok()
		.map(|target| target.display().to_string());
	Some(CliLink::Other {
		path: display,
		target,
	})
}

/// Reports the first location that holds anything, since that is the one a
/// shell finds first.
async fn status_for(
	locations: &[PathBuf],
	user_link: Option<&Path>,
	bundled: Option<&Path>,
) -> CliInstallStatus {
	let mut link = CliLink::NotInstalled;
	for location in locations {
		if let Some(found) = link_at(location, bundled).await {
			link = found;
			break;
		}
	}
	let path_hint = match &link {
		CliLink::Bundled { path } if Some(Path::new(path)) == user_link => Path::new(path)
			.parent()
			.map(|dir| dir.display().to_string()),
		_ => None,
	};
	CliInstallStatus {
		bundled: bundled.map(|path| path.display().to_string()),
		link,
		path_hint,
	}
}

pub async fn status() -> CliInstallStatus {
	status_for(
		&link_locations(),
		user_link().as_deref(),
		bundled_cli().await.as_deref(),
	)
	.await
}

/// Points `link` at `target`, replacing whatever file or link is there.
///
/// The new link is made beside the old one and renamed over it, so a failure
/// part way leaves the previous `sd` in place.
async fn replace_link(link: &Path, target: &Path) -> std::io::Result<()> {
	let dir = link
		.parent()
		.ok_or_else(|| std::io::Error::other("the link has no parent folder"))?;
	let staged = dir.join(format!(".sd.{}.tmp", std::process::id()));
	let _ = tokio::fs::remove_file(&staged).await;
	#[cfg(unix)]
	tokio::fs::symlink(target, &staged).await?;
	#[cfg(not(unix))]
	return Err(std::io::Error::new(
		std::io::ErrorKind::Unsupported,
		format!(
			"linking {} is not supported on this platform",
			target.display()
		),
	));
	if let Err(e) = tokio::fs::rename(&staged, link).await {
		let _ = tokio::fs::remove_file(&staged).await;
		return Err(e);
	}
	Ok(())
}

/// Quotes a path for `sh`, so a folder name with spaces or quotes stays
/// one argument.
#[cfg(any(target_os = "macos", all(test, unix)))]
fn shell_quote(path: &Path) -> String {
	format!("'{}'", path.display().to_string().replace('\'', r"'\''"))
}

/// Runs `command` as root after the standard macOS administrator prompt.
/// Returns false when the user cancels or the command fails.
#[cfg(target_os = "macos")]
async fn run_as_admin(command: &str, prompt: &str) -> bool {
	let output = tokio::process::Command::new("/usr/bin/osascript")
		.args([
			"-e",
			"on run argv",
			"-e",
			"do shell script (item 1 of argv) with prompt (item 2 of argv) with administrator privileges",
			"-e",
			"end run",
			command,
			prompt,
		])
		.output()
		.await;
	match output {
		Ok(output) if output.status.success() => true,
		Ok(output) => {
			info!(
				stderr = %String::from_utf8_lossy(&output.stderr).trim(),
				"Administrator prompt was refused or failed"
			);
			false
		}
		Err(e) => {
			info!(error = %e, "Could not run osascript");
			false
		}
	}
}

/// Links `/usr/local/bin/sd`, asking for an administrator when the folder is
/// not writable. Returns false when that was refused.
#[cfg(target_os = "macos")]
async fn install_system_link(bundled: &Path) -> bool {
	match replace_link(Path::new(SYSTEM_LINK), bundled).await {
		Ok(()) => return true,
		Err(e) => info!(error = %e, "Linking {SYSTEM_LINK} needs an administrator"),
	}
	run_as_admin(
		&format!(
			"mkdir -p /usr/local/bin && ln -sfn {} {SYSTEM_LINK}",
			shell_quote(bundled)
		),
		"Spacedrive wants to install the sd command line tool in /usr/local/bin.",
	)
	.await
}

/// The outcome of an install.
pub struct Installed {
	/// Administrator access was refused, so the link went to `~/.local/bin`.
	pub fell_back: bool,
	/// A command that a default PATH finds before the new link, left in place
	/// because replacing it needed the refused administrator access.
	pub shadowed_by: Option<String>,
}

/// Links `sd` to the CLI beside this daemon.
pub async fn install() -> Result<Installed, String> {
	let bundled = bundled_cli()
		.await
		.ok_or("This copy of Spacedrive has no command line tool beside its daemon to install.")?;
	// A quarantined app runs from a random read-only path that changes on
	// every launch, so a link to it would break.
	if bundled.to_string_lossy().contains("/AppTranslocation/") {
		return Err(
			"Move Spacedrive to the Applications folder, open it from there, and try again."
				.to_string(),
		);
	}

	#[cfg(target_os = "macos")]
	if install_system_link(&bundled).await {
		info!(target = %bundled.display(), "Linked {SYSTEM_LINK}");
		return Ok(Installed {
			fell_back: false,
			shadowed_by: None,
		});
	}

	let link = user_link().ok_or("Could not find the home folder")?;
	let dir = link.parent().expect("~/.local/bin/sd has a parent");
	tokio::fs::create_dir_all(dir)
		.await
		.map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
	replace_link(&link, &bundled)
		.await
		.map_err(|e| format!("Could not link {}: {e}", link.display()))?;
	info!(target = %bundled.display(), link = %link.display(), "Linked the command line tool");
	#[cfg(target_os = "macos")]
	let shadowed_by = match link_at(Path::new(SYSTEM_LINK), Some(&bundled)).await {
		Some(CliLink::Other { path, .. }) => Some(path),
		_ => None,
	};
	#[cfg(not(target_os = "macos"))]
	let shadowed_by = None;

	Ok(Installed {
		fell_back: cfg!(target_os = "macos"),
		shadowed_by,
	})
}

/// Removes every `sd` link that points at the CLI beside this daemon,
/// leaving links and binaries from anything else alone.
pub async fn uninstall() -> Result<(), String> {
	let bundled = bundled_cli().await;
	for location in link_locations() {
		if !matches!(
			link_at(&location, bundled.as_deref()).await,
			Some(CliLink::Bundled { .. })
		) {
			continue;
		}
		match tokio::fs::remove_file(&location).await {
			Ok(()) => {}
			#[cfg(target_os = "macos")]
			Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
				let removed = run_as_admin(
					&format!("rm -f {}", shell_quote(&location)),
					"Spacedrive wants to remove the sd command line tool from /usr/local/bin.",
				)
				.await;
				if !removed {
					return Err(format!(
						"Removing {} needs administrator access.",
						location.display()
					));
				}
			}
			Err(e) => return Err(format!("Could not remove {}: {e}", location.display())),
		}
		info!(link = %location.display(), "Removed the command line tool link");
	}
	Ok(())
}

#[cfg(all(test, unix))]
mod tests {
	use super::*;

	fn touch(path: &Path) {
		std::fs::write(path, b"").unwrap();
	}

	#[tokio::test]
	async fn reports_nothing_when_no_location_holds_anything() {
		let dir = tempfile::tempdir().unwrap();
		let cli = dir.path().join("sd-cli");
		touch(&cli);
		let link = dir.path().join("bin/sd");

		let status = status_for(&[link], None, Some(&cli)).await;
		assert_eq!(status.link, CliLink::NotInstalled);
		assert_eq!(status.bundled, Some(cli.display().to_string()));
	}

	#[tokio::test]
	async fn a_link_to_the_bundled_cli_is_bundled_and_replaces_a_standalone_binary() {
		let dir = tempfile::tempdir().unwrap();
		touch(&dir.path().join("sd-cli"));
		let cli = dir.path().join("sd-cli").canonicalize().unwrap();
		let link = dir.path().join("sd");
		touch(&link);

		let before = status_for(&[link.clone()], None, Some(&cli)).await;
		assert_eq!(
			before.link,
			CliLink::Other {
				path: link.display().to_string(),
				target: None
			}
		);

		replace_link(&link, &cli).await.unwrap();
		let after = status_for(&[link.clone()], Some(&link), Some(&cli)).await;
		assert_eq!(
			after.link,
			CliLink::Bundled {
				path: link.display().to_string()
			}
		);
		assert_eq!(after.path_hint, Some(dir.path().display().to_string()));
	}

	#[tokio::test]
	async fn a_link_elsewhere_reports_its_target_and_the_first_location_wins() {
		let dir = tempfile::tempdir().unwrap();
		let cli = dir.path().join("sd-cli");
		touch(&cli);
		let cli = cli.canonicalize().unwrap();
		let old = dir.path().join("Old.app-sd-cli");
		let first = dir.path().join("first-sd");
		let second = dir.path().join("second-sd");
		std::os::unix::fs::symlink(&old, &first).unwrap();
		replace_link(&second, &cli).await.unwrap();

		let status = status_for(&[first.clone(), second], None, Some(&cli)).await;
		assert_eq!(
			status.link,
			CliLink::Other {
				path: first.display().to_string(),
				target: Some(old.display().to_string())
			}
		);
		assert_eq!(status.path_hint, None);
	}

	#[test]
	fn shell_quote_keeps_spaces_and_quotes_in_one_argument() {
		assert_eq!(
			shell_quote(Path::new("/Applications/Space Drive's.app/sd-cli")),
			r"'/Applications/Space Drive'\''s.app/sd-cli'"
		);
	}
}
