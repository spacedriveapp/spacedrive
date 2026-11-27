//! # Bundled extensions
//!
//! The app ships the extensions it depends on, such as Videos, as resources
//! and installs them into `<data dir>/extensions` before the daemon starts,
//! because the daemon loads extensions only from there and only at startup.
//! An installed copy that differs from the bundled one in any file is
//! replaced, so an app update also updates its extensions. A copy that
//! matches is left alone, which keeps launches that change nothing cheap.
//! Copying is not loading: the caller restarts a daemon that was already
//! running when anything changed.

use std::path::{Path, PathBuf};

/// Installs or updates every extension under `<resource dir>/extensions`,
/// and returns whether any installed copy changed.
///
/// Each extension is written to a staging directory outside the extensions
/// directory first and swapped in with renames, so a daemon never reads a
/// half-written extension. A failure leaves that extension as it was and
/// moves on to the next.
pub async fn install_bundled(resource_dir: PathBuf, data_dir: PathBuf) -> bool {
	let result = tokio::task::spawn_blocking(move || {
		let mut changed = false;
		let bundled = resource_dir.join("extensions");
		let Ok(entries) = std::fs::read_dir(&bundled) else {
			return changed;
		};
		for entry in entries.flatten() {
			let source = entry.path();
			if !source.join("manifest.json").is_file() {
				continue;
			}
			let name = entry.file_name();
			let target = data_dir.join("extensions").join(&name);
			let staging = data_dir.join(".extensions-staging").join(&name);
			match install(&source, &target, &staging) {
				Ok(true) => {
					changed = true;
					tracing::info!(extension = ?name, "Installed bundled extension")
				}
				Ok(false) => tracing::debug!(extension = ?name, "Bundled extension is current"),
				Err(e) => {
					tracing::warn!(extension = ?name, error = %e, "Could not install bundled extension")
				}
			}
		}
		changed
	})
	.await;
	result.unwrap_or_else(|e| {
		tracing::warn!(error = %e, "Bundled extension install did not finish");
		false
	})
}

/// Returns whether the extension had to be written.
fn install(source: &Path, target: &Path, staging: &Path) -> std::io::Result<bool> {
	let mut files = read_tree(source, Path::new(""))?;
	files.sort();
	// A file the bundle dropped counts as a difference too, or an update
	// that removes a file would leave the old one installed.
	let current = read_tree(target, Path::new("")).is_ok_and(|mut installed| {
		installed.sort();
		installed == files
	});
	if current {
		return Ok(false);
	}

	if staging.exists() {
		std::fs::remove_dir_all(staging)?;
	}
	for (path, bytes) in &files {
		let dest = staging.join(path);
		if let Some(parent) = dest.parent() {
			std::fs::create_dir_all(parent)?;
		}
		std::fs::write(dest, bytes)?;
	}

	// The old copy moves aside rather than being deleted in place, so a
	// failed swap can put it back and a loader never sees a partly deleted
	// directory.
	let mut previous_name = staging.file_name().unwrap_or_default().to_os_string();
	previous_name.push(".previous");
	let previous = staging.with_file_name(previous_name);
	if previous.exists() {
		std::fs::remove_dir_all(&previous)?;
	}
	let had_previous = target.exists();
	if had_previous {
		std::fs::rename(target, &previous)?;
	} else if let Some(parent) = target.parent() {
		std::fs::create_dir_all(parent)?;
	}
	if let Err(e) = std::fs::rename(staging, target) {
		if had_previous {
			let _ = std::fs::rename(&previous, target);
		}
		return Err(e);
	}
	if had_previous {
		std::fs::remove_dir_all(&previous)?;
	}
	Ok(true)
}

fn read_tree(root: &Path, relative: &Path) -> std::io::Result<Vec<(PathBuf, Vec<u8>)>> {
	let mut files = Vec::new();
	for entry in std::fs::read_dir(root.join(relative))? {
		let entry = entry?;
		let path = relative.join(entry.file_name());
		if entry.file_type()?.is_dir() {
			files.extend(read_tree(root, &path)?);
		} else {
			files.push((path.clone(), std::fs::read(root.join(&path))?));
		}
	}
	Ok(files)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn installs_then_replaces_only_when_the_bundle_changes() {
		let dir = std::env::temp_dir().join(format!("sd-bundled-ext-{}", std::process::id()));
		let _ = std::fs::remove_dir_all(&dir);
		let source = dir.join("bundle/videos");
		let target = dir.join("data/extensions/videos");
		let staging = dir.join("data/.extensions-staging/videos");
		std::fs::create_dir_all(source.join("ui")).unwrap();
		std::fs::write(source.join("manifest.json"), "{}").unwrap();
		std::fs::write(source.join("ui/videos.js"), "v1").unwrap();

		assert!(install(&source, &target, &staging).unwrap());
		assert_eq!(
			std::fs::read_to_string(target.join("ui/videos.js")).unwrap(),
			"v1"
		);
		assert!(!install(&source, &target, &staging).unwrap());

		std::fs::write(target.join("stale.wasm"), "old").unwrap();
		std::fs::write(source.join("ui/videos.js"), "v2").unwrap();
		assert!(install(&source, &target, &staging).unwrap());
		assert_eq!(
			std::fs::read_to_string(target.join("ui/videos.js")).unwrap(),
			"v2"
		);
		assert!(!target.join("stale.wasm").exists());
		assert!(!staging.exists());
		assert!(!dir
			.join("data/.extensions-staging/videos.previous")
			.exists());

		std::fs::remove_dir_all(source.join("ui")).unwrap();
		assert!(install(&source, &target, &staging).unwrap());
		assert!(!target.join("ui").exists());
		assert!(!install(&source, &target, &staging).unwrap());

		std::fs::remove_dir_all(&dir).unwrap();
	}
}
