//! # Extension file kinds
//!
//! An extension's `manifest.json` may declare file kinds: a name, a parent
//! from the built-in [`ContentKind`] set, the file extensions and magic bytes
//! that identify it, and how the client previews it. A kind is one level deep
//! under its parent so every existing consumer of a content kind (search
//! filters, kind stats, icons) keeps understanding the parent while a loaded
//! extension refines it. The kind's id everywhere else is
//! `<extension id>:<name>`, the convention jobs already use, so two
//! extensions can both declare `raw` without colliding.
//!
//! These types live beside the registry rather than in the extension module
//! because the registry, `extensions.list` and the content identity phase all
//! read them, and all three exist in a build without the `wasm` feature.

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::domain::ContentKind;
use crate::filetype::magic::MagicBytePattern;

/// Built-in renderers a kind may name through `preview.renderer`. The client
/// registers these first; `ContentRenderer` falls back to the parent's
/// renderer for a name it does not know.
pub const BUILTIN_RENDERERS: [&str; 7] = [
	"image", "video", "audio", "mesh", "document", "text", "default",
];

/// Priority an extension kind registers with, above every built-in type, so a
/// kind that refines a built-in extension wins the lookup for it.
pub const EXTENSION_KIND_PRIORITY: u8 = 110;

/// A file kind as an extension manifest declares it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(deny_unknown_fields)]
pub struct ExtensionKind {
	/// Lowercase `[a-z0-9-]`, unique within the manifest.
	pub name: String,
	#[serde(default)]
	pub display_name: Option<String>,
	/// A built-in kind other than `unknown`, `model_entry` and `memory`.
	pub parent: ContentKind,
	/// Lowercase, no leading dot, at least one.
	pub extensions: Vec<String>,
	#[serde(default)]
	pub mime_types: Vec<String>,
	#[serde(default)]
	pub magic: Vec<MagicPatternSpec>,
	/// Absent means the parent's renderer.
	#[serde(default)]
	pub preview: Option<PreviewSpec>,
}

/// A magic byte pattern in the shape the built-in TOML definitions use.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(deny_unknown_fields)]
pub struct MagicPatternSpec {
	/// Hex bytes separated by spaces; `??` matches any byte.
	pub pattern: String,
	#[serde(default)]
	pub offset: usize,
}

/// How the client previews a kind: a built-in renderer by name, or a
/// `ui_manifest.json` `file_viewers` entry by id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum PreviewSpec {
	Renderer(String),
	Viewer(String),
}

/// A file extension two loaded extensions both claimed. The kind loaded
/// first holds the claim; the other kind's claim on that extension is
/// dropped from the lookup table and its patterns stay available to the
/// content identity phase as a tie-breaker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct KindConflict {
	/// The file extension, lowercase, no dot.
	pub extension: String,
	/// The kind id whose claim was dropped.
	pub kind: String,
	/// The kind id that holds the extension.
	pub claimed_by: String,
}

impl ExtensionKind {
	/// The kind's id under its extension: `<extension id>:<name>`.
	pub fn id(&self, extension_id: &str) -> String {
		format!("{extension_id}:{}", self.name)
	}

	/// What the host relies on beyond the shape serde enforces.
	///
	/// A parent outside the built-in set would leave the client with no
	/// renderer to fall back to, and an extension with a dot or uppercase
	/// would never match the lowercase lookup the registry does.
	pub fn validate(&self) -> Result<(), String> {
		if self.name.is_empty()
			|| !self
				.name
				.chars()
				.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
		{
			return Err(format!(
				"kind name {:?} must be lowercase [a-z0-9-]",
				self.name
			));
		}
		if matches!(
			self.parent,
			ContentKind::Unknown | ContentKind::ModelEntry | ContentKind::Memory
		) {
			return Err(format!(
				"kind {:?} cannot have {} as its parent",
				self.name, self.parent
			));
		}
		if self.extensions.is_empty() {
			return Err(format!("kind {:?} claims no file extensions", self.name));
		}
		for ext in &self.extensions {
			if ext.is_empty()
				|| !ext
					.chars()
					.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
			{
				return Err(format!(
					"kind {:?} extension {ext:?} must be lowercase with no leading dot",
					self.name
				));
			}
		}
		for magic in &self.magic {
			MagicBytePattern::from_hex_string(&magic.pattern, magic.offset, 0)
				.map_err(|e| format!("kind {:?} magic {:?}: {e}", self.name, magic.pattern))?;
		}
		if let Some(PreviewSpec::Renderer(renderer)) = &self.preview {
			if !BUILTIN_RENDERERS.contains(&renderer.as_str()) {
				return Err(format!(
					"kind {:?} names no built-in renderer {renderer:?}; one of {}",
					self.name,
					BUILTIN_RENDERERS.join(", ")
				));
			}
		}
		Ok(())
	}

	/// The magic patterns, parsed. Call after [`Self::validate`].
	pub fn magic_patterns(&self) -> Vec<MagicBytePattern> {
		self.magic
			.iter()
			.filter_map(|m| {
				MagicBytePattern::from_hex_string(&m.pattern, m.offset, EXTENSION_KIND_PRIORITY)
					.ok()
			})
			.collect()
	}
}

/// Validate every kind in one manifest together: each on its own, and the
/// names unique within the manifest.
pub fn validate_kinds(kinds: &[ExtensionKind]) -> Result<(), String> {
	let mut seen = std::collections::HashSet::new();
	for kind in kinds {
		kind.validate()?;
		if !seen.insert(kind.name.as_str()) {
			return Err(format!("kind {:?} is declared twice", kind.name));
		}
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;

	fn kind(json: &str) -> Result<ExtensionKind, String> {
		let kind: ExtensionKind = serde_json::from_str(json).map_err(|e| e.to_string())?;
		kind.validate()?;
		Ok(kind)
	}

	#[test]
	fn a_well_formed_kind_parses() {
		let raw = kind(
			r#"{"name":"raw","display_name":"RAW photo","parent":"image",
			"extensions":["cr2","nef"],"mime_types":["image/x-canon-cr2"],
			"magic":[{"pattern":"49 49 2A 00","offset":0}],"preview":{"renderer":"image"}}"#,
		)
		.unwrap();
		assert_eq!(raw.id("com.spacedrive.photos"), "com.spacedrive.photos:raw");
		assert_eq!(raw.parent, ContentKind::Image);
		assert_eq!(raw.preview, Some(PreviewSpec::Renderer("image".into())));
		assert_eq!(raw.magic_patterns().len(), 1);
	}

	#[test]
	fn the_rules_are_enforced() {
		let bad = [
			(
				r#"{"name":"Raw","parent":"image","extensions":["cr2"]}"#,
				"lowercase",
			),
			(
				r#"{"name":"raw","parent":"unknown","extensions":["cr2"]}"#,
				"parent",
			),
			(
				r#"{"name":"raw","parent":"image","extensions":[]}"#,
				"no file extensions",
			),
			(
				r#"{"name":"raw","parent":"image","extensions":[".cr2"]}"#,
				"leading dot",
			),
			(
				r#"{"name":"raw","parent":"image","extensions":["cr2"],"magic":[{"pattern":"ZZ"}]}"#,
				"magic",
			),
			(
				r#"{"name":"raw","parent":"image","extensions":["cr2"],"preview":{"renderer":"hologram"}}"#,
				"renderer",
			),
			(
				r#"{"name":"raw","parent":"image","extensions":["cr2"],"icon":"x"}"#,
				"unknown field",
			),
			(
				r#"{"name":"raw","parent":"kind-of-image","extensions":["cr2"]}"#,
				"unknown variant",
			),
		];
		for (json, reason) in bad {
			let err = kind(json).expect_err(json);
			assert!(err.contains(reason), "{json}: {err}");
		}
	}

	#[test]
	fn names_are_unique_within_a_manifest() {
		let kinds: Vec<ExtensionKind> = serde_json::from_str(
			r#"[{"name":"raw","parent":"image","extensions":["cr2"]},
			{"name":"raw","parent":"image","extensions":["nef"]}]"#,
		)
		.unwrap();
		assert!(validate_kinds(&kinds).unwrap_err().contains("twice"));
	}
}
