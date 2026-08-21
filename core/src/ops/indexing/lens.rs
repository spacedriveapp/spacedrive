//! Read-time lenses over the index.
//!
//! The walk records everything it can reach; lenses decide what ordinary
//! surfaces show. Bundle internals are the first lens: files inside a macOS
//! package (a Photos library's originals, say) are real and indexed — the
//! analyzer sums them, adapters enrich them, paths address them — but
//! browsing, collections, recents, and search treat the package as one
//! opaque item.

use std::path::Path;

/// Directory name suffixes that mark a package boundary. A path is
/// bundle-internal when any strict ancestor carries one of these suffixes;
/// the package directory itself is an ordinary entry.
const BUNDLE_SUFFIXES: &[&str] = &[".photoslibrary"];

/// Whether ordinary surfaces should treat this path as hidden inside a
/// package. Pure function of the path, so read sites need no index state
/// and the verdict never drifts from what is on disk.
pub fn is_bundle_internal(path: &Path) -> bool {
	let Some(parent) = path.parent() else {
		return false;
	};
	parent.components().any(|component| {
		let name = component.as_os_str().to_string_lossy();
		BUNDLE_SUFFIXES
			.iter()
			.any(|suffix| name.to_ascii_lowercase().ends_with(suffix))
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn bundle_contents_are_internal() {
		for path in [
			"/Users/james/Pictures/Photos Library.photoslibrary/originals/A/B/IMG_0001.HEIC",
			"/Users/james/Pictures/Photos Library.photoslibrary/originals",
			"/Volumes/Backup/Old.PhotosLibrary/database/Photos.sqlite",
		] {
			assert!(is_bundle_internal(Path::new(path)), "{path}");
		}
	}

	#[test]
	fn the_bundle_itself_is_not() {
		assert!(!is_bundle_internal(Path::new(
			"/Users/james/Pictures/Photos Library.photoslibrary"
		)));
	}

	#[test]
	fn ordinary_paths_are_not() {
		for path in [
			"/Users/james/Pictures/IMG_0001.HEIC",
			"/Users/james/Documents/photoslibrary notes.md",
			"/",
		] {
			assert!(!is_bundle_internal(Path::new(path)), "{path}");
		}
	}
}
