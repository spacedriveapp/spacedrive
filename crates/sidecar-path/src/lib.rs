//! The canonical sidecar tree layout:
//! `content/{h0}/{h1}/{content_uuid}/{kind_dir}/{variant}.{ext}`
//! where `h0`/`h1` are the first two byte-pairs of the lowercase hex UUID.
//!
//! Every process that touches the sidecar tree resolves paths through this
//! crate — the daemon writes with it, and the serving processes (which do not
//! link the full core) read with it — so the layout cannot drift between
//! writer and readers.

use std::path::PathBuf;
use uuid::Uuid;

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
		"gaussian_splat" => Some("gaussian_splats"),
		_ => None,
	}
}

/// Shard directories for a content UUID: the first two byte-pairs of the
/// lowercase hex representation with hyphens removed.
pub fn compute_shards(content_uuid: &Uuid) -> (String, String) {
	let hex = content_uuid.simple().to_string();
	(hex[0..2].to_string(), hex[2..4].to_string())
}

/// Path of a sidecar file relative to the library's `sidecars` directory.
pub fn relative_path(
	content_uuid: &Uuid,
	kind_dir: &str,
	variant: &str,
	extension: &str,
) -> PathBuf {
	let (h0, h1) = compute_shards(content_uuid);
	let mut path = PathBuf::from("content");
	path.push(h0);
	path.push(h1);
	path.push(content_uuid.to_string());
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
			PathBuf::from("content/ab/cd/abcd1234-5678-90ab-cdef-123456789012/thumbs/grid@2x.webp")
		);
	}

	#[test]
	fn irregular_plurals_map_correctly() {
		assert_eq!(kind_directory("proxy"), Some("proxies"));
		assert_eq!(kind_directory("embeddings"), Some("embeddings"));
		assert_eq!(kind_directory("transcript"), Some("transcript"));
		assert_eq!(kind_directory("bogus"), None);
	}
}
