//! # Bundled extensions
//!
//! The app ships the extensions it depends on, such as Videos, as resources
//! and installs them into `<data dir>/extensions` before the daemon starts,
//! because the daemon loads extensions only from there and only at startup.
//! An installed copy that differs from the bundled one in any file is
//! replaced, so an app update also updates its extensions. A copy that
//! matches is left alone, which keeps launches that change nothing cheap.

use std::path::{Path, PathBuf};

/// Installs or updates every extension under `<resource dir>/extensions`.
///
/// Each extension is written to a staging directory outside the extensions
/// directory first and then renamed into place, so a daemon starting at the
/// same time never loads a half-written extension. A failure leaves that
/// extension as it was and moves on to the next.
pub async fn install_bundled(resource_dir: PathBuf, data_dir: PathBuf) {
	let result = tokio::task::spawn_blocking(move || {
		let bundled = resource_dir.join("extensions");
		let Ok(entries) = std::fs::read_dir(&bundled) else {
			return;
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
				Ok(true) => tracing::info!(extension = ?name, "Installed bundled extension"),
				Ok(false) => tracing::debug!(extension = ?name, "Bundled extension is current"),
				Err(e) => {
					tracing::warn!(extension = ?name, error = %e, "Could not install bundled extension")
				}
			}
		}
	})
	.await;
	if let Err(e) = result {
		tracing::warn!(error = %e, "Bundled extension install did not finish");
	}
}

/// Returns whether the extension had to be written.
fn install(source: &Path, target: &Path, staging: &Path) -> std::io::Result<bool> {
	let files = read_tree(source, Path::new(""))?;
	let current = files
		.iter()
		.all(|(path, bytes)| std::fs::read(target.join(path)).is_ok_and(|b| &b == bytes));
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

	if target.exists() {
		std::fs::remove_dir_all(target)?;
	}
	if let Some(parent) = target.parent() {
		std::fs::create_dir_all(parent)?;
	}
	std::fs::rename(staging, target)?;
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

		std::fs::remove_dir_all(&dir).unwrap();
	}
}
