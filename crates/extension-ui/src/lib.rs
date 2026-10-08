//! # Extension UI manifest
//!
//! An extension's `ui_manifest.json` declares what it contributes to the
//! client. This crate reads the one section the daemon acts on today,
//! `file_viewers`: each entry names a viewer by `id` and the ES module
//! (`bundle`) the client mounts for a file kind whose `preview.viewer` names
//! it. The HTTP servers that serve those bundles (sd-server and the desktop
//! app's local server) do not link the full core, so the parser and the path
//! resolution live here, the way `sd-sidecar-path` shares the sidecar layout.
//!
//! Only a declared bundle is ever served. `resolve_bundle` answers a request
//! for `/extension/<id>/<path>` with the file on disk when `<path>` is one of
//! the extension's `file_viewers[].bundle` values and nothing else, so a
//! `config.json` or the `.wasm` beside it stays private.

use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The file name beside `manifest.json`.
pub const UI_MANIFEST_FILE: &str = "ui_manifest.json";

/// The sections of `ui_manifest.json` the daemon reads. The other sections
/// (sidebar, context menu, toolbar, search filters) stay unread until the UI
/// contributions work, so unknown fields are accepted here.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiManifest {
	#[serde(default)]
	pub file_viewers: Vec<FileViewer>,
}

/// A viewer the client can mount for a file kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileViewer {
	/// What a kind's `preview.viewer` names.
	pub id: String,
	/// One ES module, as a path inside the extension directory, exporting
	/// `mount(el, ctx)`.
	pub bundle: String,
}

impl UiManifest {
	/// Parse the file's text.
	pub fn parse(text: &str) -> Result<Self, String> {
		serde_json::from_str(text).map_err(|e| e.to_string())
	}

	/// Read `ui_manifest.json` from an extension directory. An absent file
	/// is an empty manifest; an unreadable one is an error, so a typo does
	/// not silently drop every viewer.
	pub async fn read_from(extension_dir: &Path) -> Result<Self, String> {
		let path = extension_dir.join(UI_MANIFEST_FILE);
		match tokio::fs::read_to_string(&path).await {
			Ok(text) => Self::parse(&text).map_err(|e| format!("{}: {e}", path.display())),
			Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
			Err(e) => Err(format!("{}: {e}", path.display())),
		}
	}

	/// What the daemon relies on beyond the shape serde enforces: ids are
	/// unique and every bundle is a relative path that stays inside the
	/// extension directory, since the HTTP route joins it under that
	/// directory.
	pub fn validate(&self) -> Result<(), String> {
		let mut seen = std::collections::HashSet::new();
		for viewer in &self.file_viewers {
			if viewer.id.is_empty() {
				return Err("a file viewer has an empty id".into());
			}
			if !seen.insert(viewer.id.as_str()) {
				return Err(format!("file viewer {:?} is declared twice", viewer.id));
			}
			if !is_inside_relative(&viewer.bundle) {
				return Err(format!(
					"file viewer {:?} bundle {:?} must be a relative path inside the extension directory",
					viewer.id, viewer.bundle
				));
			}
		}
		Ok(())
	}

	/// The viewer with this id.
	pub fn viewer(&self, id: &str) -> Option<&FileViewer> {
		self.file_viewers.iter().find(|v| v.id == id)
	}

	/// Whether `path`, as a request names it, is one of the declared bundles.
	pub fn declares_bundle(&self, path: &str) -> bool {
		self.file_viewers.iter().any(|v| v.bundle == path)
	}
}

/// A relative path with only normal components, so joining it under a
/// directory cannot escape that directory.
fn is_inside_relative(path: &str) -> bool {
	let path = Path::new(path);
	!path.as_os_str().is_empty() && path.components().all(|c| matches!(c, Component::Normal(_)))
}

/// The directory under `<data dir>/extensions` holding the extension with
/// this manifest id. The directory name need not match the id, so each
/// `manifest.json` is read for its `id`; a directory named after the id is
/// tried first because that is the common case.
pub async fn extension_dir(data_dir: &Path, extension_id: &str) -> Option<PathBuf> {
	let extensions = data_dir.join("extensions");
	if is_inside_relative(extension_id) && !extension_id.contains(['/', '\\']) {
		let direct = extensions.join(extension_id);
		if manifest_id(&direct).await.as_deref() == Some(extension_id) {
			return Some(direct);
		}
	}
	let mut entries = tokio::fs::read_dir(&extensions).await.ok()?;
	while let Ok(Some(entry)) = entries.next_entry().await {
		let dir = entry.path();
		if manifest_id(&dir).await.as_deref() == Some(extension_id) {
			return Some(dir);
		}
	}
	None
}

async fn manifest_id(dir: &Path) -> Option<String> {
	let text = tokio::fs::read_to_string(dir.join("manifest.json"))
		.await
		.ok()?;
	let manifest: serde_json::Value = serde_json::from_str(&text).ok()?;
	manifest.get("id")?.as_str().map(str::to_string)
}

/// The file behind `/extension/<extension_id>/<path>`: the bundle on disk
/// when the extension's `ui_manifest.json` declares `path` as a viewer
/// bundle, `None` for anything else. A declared bundle whose file is gone
/// still resolves to its path; the caller's open fails and reports that.
pub async fn resolve_bundle(data_dir: &Path, extension_id: &str, path: &str) -> Option<PathBuf> {
	if !is_inside_relative(path) {
		return None;
	}
	let dir = extension_dir(data_dir, extension_id).await?;
	let ui = UiManifest::read_from(&dir).await.ok()?;
	if ui.validate().is_err() || !ui.declares_bundle(path) {
		return None;
	}
	Some(dir.join(path))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn the_other_sections_are_ignored_and_the_viewer_fields_are_read() {
		let ui = UiManifest::parse(
			r#"{"sidebar":{"section":"Photos"},
			"file_viewers":[{"id":"photo_viewer","bundle":"ui/photo_viewer.js",
			"mime_types":["image/jpeg"],"component":"photo_viewer","features":{"slideshow":true}}]}"#,
		)
		.unwrap();
		assert_eq!(ui.validate(), Ok(()));
		assert_eq!(
			ui.viewer("photo_viewer").unwrap().bundle,
			"ui/photo_viewer.js"
		);
		assert!(ui.declares_bundle("ui/photo_viewer.js"));
		assert!(!ui.declares_bundle("manifest.json"));
	}

	#[test]
	fn a_bundle_that_leaves_the_directory_is_refused() {
		for bundle in ["../x.js", "/etc/passwd", "ui/../../x.js", ""] {
			let ui = UiManifest {
				file_viewers: vec![FileViewer {
					id: "v".into(),
					bundle: bundle.into(),
				}],
			};
			assert!(ui.validate().is_err(), "{bundle:?}");
		}
		let twice = UiManifest {
			file_viewers: vec![
				FileViewer {
					id: "v".into(),
					bundle: "a.js".into(),
				},
				FileViewer {
					id: "v".into(),
					bundle: "b.js".into(),
				},
			],
		};
		assert!(twice.validate().unwrap_err().contains("twice"));
	}

	#[tokio::test]
	async fn only_a_declared_bundle_resolves_and_the_id_need_not_match_the_directory() {
		let data_dir = tempfile::tempdir().unwrap();
		let dir = data_dir.path().join("extensions/some-folder");
		std::fs::create_dir_all(dir.join("ui")).unwrap();
		std::fs::write(
			dir.join("manifest.json"),
			r#"{"id":"com.example.viewer","name":"x","version":"1","wasm_file":"x.wasm"}"#,
		)
		.unwrap();
		std::fs::write(
			dir.join(UI_MANIFEST_FILE),
			r#"{"file_viewers":[{"id":"v","bundle":"ui/v.js"}]}"#,
		)
		.unwrap();
		std::fs::write(dir.join("ui/v.js"), "export function mount() {}").unwrap();

		assert_eq!(
			resolve_bundle(data_dir.path(), "com.example.viewer", "ui/v.js").await,
			Some(dir.join("ui/v.js"))
		);
		for path in [
			"manifest.json",
			"ui/../manifest.json",
			"../other/x.js",
			"ui/w.js",
		] {
			assert_eq!(
				resolve_bundle(data_dir.path(), "com.example.viewer", path).await,
				None,
				"{path:?}"
			);
		}
		assert_eq!(
			resolve_bundle(data_dir.path(), "some-folder", "ui/v.js").await,
			None,
			"the directory name is not the id"
		);
		assert_eq!(
			resolve_bundle(data_dir.path(), "../some-folder", "ui/v.js").await,
			None
		);
	}
}
