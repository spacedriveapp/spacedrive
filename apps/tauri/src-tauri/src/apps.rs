//! Spacedrive's own apps, launched from the Apps menu.
//!
//! An app is a separate binary shipped beside this one with a window of its
//! own. It finds the daemon the same way every other client does, so nothing
//! needs handing over at launch: the explorer publishes where it is looking
//! and the app follows over the daemon's navigation focus.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::Mutex;

/// One launchable app: its menu item id, its label, and the binary behind it.
pub struct AppEntry {
	pub id: &'static str,
	pub title: &'static str,
	binary: &'static str,
}

/// Everything the Apps menu offers.
pub const APPS: &[AppEntry] = &[AppEntry {
	id: "app-photos",
	title: "Photos",
	binary: "spacedrive-native",
}];

pub fn find(id: &str) -> Option<&'static AppEntry> {
	APPS.iter().find(|app| app.id == id)
}

/// Resolve a binary shipped beside this executable.
///
/// Tauri's `externalBin` bundles with a target triple suffix; dev builds have
/// the plain name in the workspace target directory. Both are checked so a
/// launch works either way.
pub fn bundled_binary(stem: &str) -> Result<PathBuf, String> {
	let exe = std::env::current_exe().map_err(|e| format!("Failed to get current exe: {e}"))?;
	let dir = exe.parent().ok_or("No parent directory for exe")?;

	let with_triple = format!(
		"{stem}-{}{}",
		env!("SD_TARGET_TRIPLE"),
		std::env::consts::EXE_SUFFIX
	);
	let plain = format!("{stem}{}", std::env::consts::EXE_SUFFIX);

	[&with_triple, &plain]
		.iter()
		.map(|name| dir.join(name))
		.find(|path| path.exists())
		.ok_or_else(|| {
			format!("{stem} binary not found. Checked {with_triple} and {plain} in {dir:?}")
		})
}

/// Apps launched from this shell, so opening one twice raises the window that
/// already exists instead of a duplicate process.
#[derive(Default)]
pub struct RunningApps(Mutex<HashMap<&'static str, Child>>);

impl RunningApps {
	/// Start `app`, or bring it to the front if it is already running. Returns
	/// whether a process was spawned.
	pub fn launch(&self, app: &'static AppEntry) -> Result<bool, String> {
		let mut running = self
			.0
			.lock()
			.unwrap_or_else(|poisoned| poisoned.into_inner());

		// A child that has exited is reaped here rather than on a timer: the
		// only moment the answer matters is the next launch.
		if let Some(child) = running.get_mut(app.id) {
			match child.try_wait() {
				Ok(Some(_)) => {
					running.remove(app.id);
				}
				Ok(None) => {
					raise(child.id());
					return Ok(false);
				}
				Err(e) => return Err(format!("Failed to check {}: {e}", app.title)),
			}
		}

		let binary = bundled_binary(app.binary)?;
		let child = Command::new(&binary)
			.spawn()
			.map_err(|e| format!("Failed to launch {}: {e}", app.title))?;
		running.insert(app.id, child);
		Ok(true)
	}
}

/// Bring a running app's window to the front. The menu asking belongs to this
/// app, which is active while its menu is in use, so macOS lets it hand
/// activation over.
#[cfg(target_os = "macos")]
fn raise(pid: u32) {
	if !sd_desktop_macos::activate_process(pid) {
		tracing::warn!("[Apps] No application runs under pid {pid}");
	}
}

/// Other platforms leave the running window where it is.
#[cfg(not(target_os = "macos"))]
fn raise(_pid: u32) {}
