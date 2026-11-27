//! The canonical layout of derived media:
//! `sources/<source>/derived/{h0}/{h1}/{content_uuid}/{kind_dir}/{variant}.{ext}`
//! under the daemon's data directory, where `h0`/`h1` are the first two
//! byte-pairs of the lowercase hex UUID.
//!
//! Each content has one copy, under one of the sources that hold it, so a
//! reader that knows only the content uuid probes every source's `derived`
//! directory. Libraries written before the move kept the same tree at
//! `<library>/sidecars/content`, which readers still probe last until the
//! daemon has adopted it.
//!
//! Every process that touches the tree resolves paths through this crate. The
//! daemon writes with it, and the serving processes, which do not link the
//! full core, read with it, so the layout cannot drift between writer and
//! readers.

use std::path::{Path, PathBuf};
use uuid::Uuid;

/// The directory under a source's directory that holds its derived media.
/// Not `sidecars`, which would read as a sibling of the source's thumbnail
/// store `sidecars.db` rather than a different thing.
pub const DERIVED_DIR: &str = "derived";

/// A source's derived media directory under the daemon's `sources`
/// directory.
pub fn derived_root(sources_dir: &Path, source_id: &Uuid) -> PathBuf {
	sources_dir
		.join(source_id.simple().to_string())
		.join(DERIVED_DIR)
}

/// Where a library written before the move kept its derived media.
pub fn legacy_root(library_dir: &Path) -> PathBuf {
	library_dir.join("sidecars").join("content")
}

/// The file holding a sidecar, found by probing every source's derived
/// directory under `sources_dir` and then the library's legacy tree.
pub fn locate(sources_dir: &Path, library_dir: &Path, relative: &Path) -> Option<PathBuf> {
	let mut roots: Vec<PathBuf> = std::fs::read_dir(sources_dir)
		.into_iter()
		.flatten()
		.flatten()
		.map(|entry| entry.path().join(DERIVED_DIR))
		.collect();
	roots.sort();
	roots.push(legacy_root(library_dir));
	roots
		.into_iter()
		.map(|root| root.join(relative))
		.find(|path| path.is_file())
}

/// Directory name for a sidecar kind string, or `None` for an unknown kind.
///
/// Kind strings match `SidecarKind::as_str` in the core; the directory names
/// are not derivable from them mechanically (`proxy` → `proxies`,
/// `embeddings` and `transcript` stay as-is), which is why this mapping is
/// the single authority.
pub fn kind_directory(kind: &str) -> Option<&'static str> {
	match kind {
		"thumb" => Some("thumbs"),
		"thumbstrip" => Some("thumbstrips"),
		"proxy" => Some("proxies"),
		"embeddings" => Some("embeddings"),
		"ocr" => Some("ocr"),
		"transcript" => Some("transcript"),
		"shots" => Some("shots"),
		"keyframe" => Some("keyframes"),
		"gaussian_splat" => Some("gaussian_splats"),
		"audio" => Some("audio"),
		_ => None,
	}
}

/// The kind a directory under a content uuid holds, the inverse of
/// [`kind_directory`], so a reader listing the tree names what it finds the
/// way writers addressed it.
pub fn directory_kind(directory: &str) -> Option<&'static str> {
	[
		"thumb",
		"thumbstrip",
		"proxy",
		"embeddings",
		"ocr",
		"transcript",
		"shots",
		"keyframe",
		"gaussian_splat",
		"audio",
	]
	.into_iter()
	.find(|kind| kind_directory(kind) == Some(directory))
}

/// Directory for a kind an extension declared: under `extensions/<id>/` so
/// an extension's kinds never collide with the core's or another's.
pub fn extension_kind_directory(extension_id: &str, kind: &str) -> String {
	format!("extensions/{extension_id}/{kind}")
}

/// Shard directories for a content UUID: the first two byte-pairs of the
/// lowercase hex representation with hyphens removed.
pub fn compute_shards(content_uuid: &Uuid) -> (String, String) {
	let hex = content_uuid.simple().to_string();
	(hex[0..2].to_string(), hex[2..4].to_string())
}

/// Directory of every sidecar of one content uuid, relative to a derived
/// root.
pub fn content_directory(content_uuid: &Uuid) -> PathBuf {
	let (h0, h1) = compute_shards(content_uuid);
	let mut path = PathBuf::from(h0);
	path.push(h1);
	path.push(content_uuid.to_string());
	path
}

/// Path of a sidecar file relative to a derived root.
pub fn relative_path(
	content_uuid: &Uuid,
	kind_dir: &str,
	variant: &str,
	extension: &str,
) -> PathBuf {
	let mut path = content_directory(content_uuid);
	path.push(kind_dir);
	path.push(format!("{variant}.{extension}"));
	path
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn shards_are_first_two_byte_pairs() {
		let uuid = Uuid::parse_str("abcd1234-5678-90ab-cdef-123456789012").unwrap();
		assert_eq!(compute_shards(&uuid), ("ab".to_string(), "cd".to_string()));
	}

	#[test]
	fn relative_path_matches_layout() {
		let uuid = Uuid::parse_str("abcd1234-5678-90ab-cdef-123456789012").unwrap();
		assert_eq!(
			relative_path(&uuid, "thumbs", "grid@2x", "webp"),
			PathBuf::from("ab/cd/abcd1234-5678-90ab-cdef-123456789012/thumbs/grid@2x.webp")
		);
	}

	#[test]
	fn extension_kinds_sit_under_their_extension() {
		let uuid = Uuid::parse_str("abcd1234-5678-90ab-cdef-123456789012").unwrap();
		assert_eq!(
			relative_path(
				&uuid,
				&extension_kind_directory("com.spacedrive.photos", "faces"),
				"default",
				"json"
			),
			PathBuf::from(
				"ab/cd/abcd1234-5678-90ab-cdef-123456789012/extensions/com.spacedrive.photos/faces/default.json"
			)
		);
	}

	#[test]
	fn locate_probes_every_source_then_the_legacy_tree() {
		let data = std::env::temp_dir().join(format!("sd-sidecar-path-{}", std::process::id()));
		let sources = data.join("sources");
		let library = data.join("libraries").join("a.sdlibrary");
		let uuid = Uuid::parse_str("abcd1234-5678-90ab-cdef-123456789012").unwrap();
		let relative = relative_path(&uuid, "proxies", "720p", "mp4");
		let (a, b) = (Uuid::from_u128(1), Uuid::from_u128(2));
		std::fs::create_dir_all(derived_root(&sources, &a)).unwrap();
		assert_eq!(locate(&sources, &library, &relative), None);

		let legacy = legacy_root(&library).join(&relative);
		std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
		std::fs::write(&legacy, b"old").unwrap();
		assert_eq!(locate(&sources, &library, &relative), Some(legacy));

		let adopted = derived_root(&sources, &b).join(&relative);
		std::fs::create_dir_all(adopted.parent().unwrap()).unwrap();
		std::fs::write(&adopted, b"new").unwrap();
		assert_eq!(locate(&sources, &library, &relative), Some(adopted));
		std::fs::remove_dir_all(&data).unwrap();
	}

	#[test]
	fn irregular_plurals_map_correctly() {
		assert_eq!(kind_directory("proxy"), Some("proxies"));
		assert_eq!(kind_directory("embeddings"), Some("embeddings"));
		assert_eq!(kind_directory("transcript"), Some("transcript"));
		assert_eq!(kind_directory("audio"), Some("audio"));
		assert_eq!(kind_directory("shots"), Some("shots"));
		assert_eq!(kind_directory("keyframe"), Some("keyframes"));
		assert_eq!(kind_directory("bogus"), None);
	}

	#[test]
	fn every_kind_directory_maps_back_to_its_kind() {
		for directory in [
			"thumbs",
			"proxies",
			"keyframes",
			"gaussian_splats",
			"transcript",
		] {
			let kind = directory_kind(directory).unwrap();
			assert_eq!(kind_directory(kind), Some(directory));
		}
		assert_eq!(directory_kind("extensions"), None);
	}
}
