//! The extension manifest and the loaded-plugin record.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

/// Extension manifest (`manifest.json`).
///
/// Unknown fields are refused rather than ignored, so a grant the host does
/// not understand fails at load instead of silently granting nothing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtensionManifest {
	pub id: String,
	pub name: String,
	pub version: String,
	#[serde(default)]
	pub description: String,
	#[serde(default)]
	pub author: String,
	#[serde(default)]
	pub homepage: Option<String>,
	#[serde(default)]
	pub min_core_version: Option<String>,
	#[serde(default)]
	pub required_features: Vec<String>,

	/// WASM file path (relative to manifest)
	pub wasm_file: PathBuf,

	/// What the extension may read and write. A manifest that declares none
	/// can still register and run jobs; every host operation that touches
	/// library data checks here first.
	#[serde(default)]
	pub permissions: ManifestPermissions,

	/// AI models the extension wants installed. Declaration only: no host
	/// side downloads or serves them yet.
	#[serde(default)]
	pub models: Vec<ManifestModel>,

	/// Configuration schema (JSON Schema)
	#[serde(default)]
	pub config_schema: Option<serde_json::Value>,
}

/// Permission declaration in manifest.
///
/// Sidecar kinds are per extension: `write_sidecars: ["faces"]` grants
/// `faces` under this extension's own namespace, never another's. An
/// extension may always read back the kinds it writes.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestPermissions {
	/// Read records (and their bytes) from the library's source stores.
	#[serde(default)]
	pub read_records: Option<ReadRecords>,
	#[serde(default)]
	pub read_sidecars: Vec<String>,
	#[serde(default)]
	pub write_sidecars: Vec<String>,
	#[serde(default)]
	pub write_tags: bool,
	/// Namespaces this extension may write custom fields under.
	#[serde(default)]
	pub write_custom_fields: Vec<String>,
	#[serde(default)]
	pub dispatch_jobs: bool,
	#[serde(default)]
	pub use_models: Vec<UseModel>,
}

impl ManifestPermissions {
	pub fn can_read_sidecar(&self, kind: &str) -> bool {
		self.read_sidecars.iter().any(|k| k == kind) || self.can_write_sidecar(kind)
	}

	pub fn can_write_sidecar(&self, kind: &str) -> bool {
		self.write_sidecars.iter().any(|k| k == kind)
	}

	/// Whether a record with this extension (lowercase, no dot) is readable.
	pub fn can_read_record(&self, extension: Option<&str>) -> bool {
		match &self.read_records {
			None => false,
			Some(ReadRecords { glob: None }) => true,
			Some(ReadRecords { glob: Some(glob) }) => {
				let Some(extension) = extension else {
					return false;
				};
				glob_extensions(glob)
					.map(|allowed| allowed.iter().any(|e| e == extension))
					.unwrap_or(false)
			}
		}
	}
}

/// The extensions named by a glob of the form `**/*.{jpg,png}` or `*.jpg`.
///
/// Only the extension list is honored today; the directory part of the glob
/// is accepted and ignored, since records are addressed by uuid rather than
/// by path when an extension asks for them.
fn glob_extensions(glob: &str) -> Option<Vec<String>> {
	let tail = glob.rsplit('/').next()?;
	let tail = tail.strip_prefix("*.")?;
	let list = tail
		.strip_prefix('{')
		.and_then(|t| t.strip_suffix('}'))
		.unwrap_or(tail);
	Some(
		list.split(',')
			.map(|e| e.trim().to_lowercase())
			.filter(|e| !e.is_empty())
			.collect(),
	)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadRecords {
	/// A glob over file names; `None` grants every record.
	#[serde(default)]
	pub glob: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UseModel {
	pub category: String,
	#[serde(default)]
	pub preference: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestModel {
	pub name: String,
	pub category: String,
	pub source: serde_json::Value,
	#[serde(default)]
	pub description: String,
}

/// Loaded plugin instance
pub struct LoadedPlugin {
	pub id: String,
	/// Directory under the plugin directory it was loaded from, which the
	/// manifest id need not match.
	pub dir_name: String,
	pub manifest: Arc<ExtensionManifest>,
	pub loaded_at: DateTime<Utc>,
	/// Set by the runtime when a guest call trapped; see `PluginRuntime`.
	pub poisoned: Arc<AtomicBool>,
	pub runtime: Arc<Mutex<super::manager::PluginRuntime>>,
}

/// Alias for consistency with other code
pub type PluginManifest = ExtensionManifest;

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn photos_manifest_permissions_parse_strictly() {
		let manifest: ExtensionManifest =
			serde_json::from_str(include_str!("../../../../extensions/photos/manifest.json"))
				.unwrap();
		let perms = &manifest.permissions;
		assert!(perms.can_read_record(Some("jpg")));
		assert!(!perms.can_read_record(Some("txt")));
		assert!(perms.can_write_sidecar("faces"));
		assert!(perms.can_read_sidecar("faces"));
		assert!(perms.can_read_sidecar("exif"));
		assert!(!perms.can_write_sidecar("exif"));
		assert_eq!(manifest.models.len(), 2);
	}

	#[test]
	fn an_unknown_permission_is_refused() {
		let err =
			serde_json::from_str::<ManifestPermissions>(r#"{"methods": ["query:"]}"#).unwrap_err();
		assert!(err.to_string().contains("unknown field"));
	}

	#[test]
	fn a_glob_without_a_list_names_one_extension() {
		let perms = ManifestPermissions {
			read_records: Some(ReadRecords {
				glob: Some("*.PDF".into()),
			}),
			..Default::default()
		};
		assert!(perms.can_read_record(Some("pdf")));
		assert!(!perms.can_read_record(None));
	}
}
