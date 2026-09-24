//! The source stores that hold what is beneath a path.
//!
//! A registered source's store holds the records beneath its root, except
//! beneath a source nested inside it: the innermost registered source keeps a
//! path's records, so a nested source's files live in its own store rather
//! than the outer one's. What is beneath a path is therefore the innermost
//! source holding the path, read from the path down, and every source nested
//! beneath the path, read whole.

use std::path::Path;

use crate::context::CoreContext;
use crate::domain::SdPath;
use crate::ops::indexing::{volume_index::SourceStatus, VolumeIndex};
use crate::volume::VolumeManager;

/// One store a path reaches, and how its files sit beneath the path.
#[derive(Debug, Clone)]
pub struct Reach {
	pub source: SourceStatus,
	/// The directory the store is read from, relative to the source's root:
	/// the path itself for the source holding it, "" for one nested beneath.
	pub scope: String,
	/// Where the store's files sit relative to the path: "" for the source
	/// holding the path, and a nested source's own place beneath the path.
	pub prefix: String,
}

/// The innermost source holding `path`, among those with a root on this
/// machine.
pub fn innermost_source<'a>(sources: &'a [SourceStatus], path: &Path) -> Option<&'a SourceStatus> {
	sources
		.iter()
		.filter(|source| !source.root.as_os_str().is_empty() && path.starts_with(&source.root))
		.max_by_key(|source| source.root.as_os_str().len())
}

/// The stores beneath a path on this device, in root order. A path on another
/// device reaches none.
pub async fn stores_beneath(context: &CoreContext, path: &SdPath) -> Vec<Reach> {
	stores_beneath_in(&context.volume_manager, context.volume_index(), path).await
}

/// [`stores_beneath`] from the volume registry and the index alone, which is
/// all it reads, so a read-only context can ask.
pub async fn stores_beneath_in(
	volumes: &VolumeManager,
	index: &VolumeIndex,
	path: &SdPath,
) -> Vec<Reach> {
	let Some(path) = path.as_local_path() else {
		return Vec::new();
	};
	// The volume decides how a path is spelled, so a path reached through an
	// alias such as /Users/me is rewritten before it meets any root.
	let path = match volumes.locate_path(path).await {
		Some((_, spelled)) => spelled,
		None => path.to_path_buf(),
	};
	reach(mounted_sources(index), &path)
}

/// Every store with a root on this machine, each read whole, in root order.
pub fn every_store(context: &CoreContext) -> Vec<Reach> {
	every_store_in(context.volume_index())
}

/// [`every_store`] from the index alone.
pub fn every_store_in(index: &VolumeIndex) -> Vec<Reach> {
	mounted_sources(index)
		.into_iter()
		.map(|source| Reach {
			source,
			scope: String::new(),
			prefix: String::new(),
		})
		.collect()
}

/// Sources with a root on this machine, ordered by it. A source whose drive
/// is not mounted has no root to build paths from.
fn mounted_sources(index: &VolumeIndex) -> Vec<SourceStatus> {
	let mut sources: Vec<SourceStatus> = index
		.sources()
		.into_iter()
		.filter(|source| !source.root.as_os_str().is_empty())
		.collect();
	sources.sort_by(|a, b| a.root.cmp(&b.root).then(a.id.cmp(&b.id)));
	sources
}

fn reach(sources: Vec<SourceStatus>, path: &Path) -> Vec<Reach> {
	let innermost = innermost_source(&sources, path).map(|source| source.id);
	sources
		.into_iter()
		.filter_map(|source| {
			if Some(source.id) == innermost {
				let scope = relative(path, &source.root)?;
				Some(Reach {
					source,
					scope,
					prefix: String::new(),
				})
			} else if source.root.starts_with(path) {
				let prefix = relative(&source.root, path)?;
				Some(Reach {
					source,
					scope: String::new(),
					prefix,
				})
			} else {
				None
			}
		})
		.collect()
}

/// `path` beneath `base` the way stores spell it: relative, with forward
/// slashes whatever the host writes.
fn relative(path: &Path, base: &Path) -> Option<String> {
	Some(
		path.strip_prefix(base)
			.ok()?
			.to_str()?
			.replace(std::path::MAIN_SEPARATOR, "/"),
	)
}

/// A registered, attached source at `root`.
#[cfg(test)]
pub(super) fn source(root: &str) -> SourceStatus {
	SourceStatus {
		id: uuid::Uuid::new_v4(),
		root: std::path::PathBuf::from(root),
		volume_uuid: None,
		attached: true,
		restored: true,
		last_seen_secs: 0,
		entry_count: None,
		total_bytes: None,
		directory: None,
		thumbs_path: None,
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::path::PathBuf;

	fn reached(sources: &[SourceStatus], path: &str) -> Vec<(String, String, String)> {
		reach(sources.to_vec(), Path::new(path))
			.into_iter()
			.map(|reach| {
				(
					reach.source.root.display().to_string(),
					reach.scope,
					reach.prefix,
				)
			})
			.collect()
	}

	/// A folder is read from the innermost source holding it, and a source
	/// nested beneath the folder is read whole, since its files are in its own
	/// store. A folder above every source reaches each one beneath it.
	#[test]
	fn a_path_reaches_the_source_holding_it_and_those_nested_beneath() {
		let sources = [source("/A"), source("/A/m"), source("/B")];
		let owned = |root: &str, scope: &str, prefix: &str| {
			(root.to_string(), scope.to_string(), prefix.to_string())
		};

		assert_eq!(reached(&sources, "/A/x"), [owned("/A", "x", "")]);
		assert_eq!(
			reached(&sources, "/A"),
			[owned("/A", "", ""), owned("/A/m", "", "m")]
		);
		assert_eq!(reached(&sources, "/A/m/deep"), [owned("/A/m", "deep", "")]);
		assert_eq!(
			reached(&sources, "/"),
			[
				owned("/A", "", "A"),
				owned("/A/m", "", "A/m"),
				owned("/B", "", "B")
			]
		);
		assert!(reached(&sources, "/C").is_empty());
	}

	#[test]
	fn the_innermost_source_skips_one_with_no_root() {
		let sources = [source(""), source("/A"), source("/A/m")];
		assert_eq!(
			innermost_source(&sources, Path::new("/A/m/x")).map(|source| &source.root),
			Some(&PathBuf::from("/A/m"))
		);
		assert!(innermost_source(&sources, Path::new("/C")).is_none());
	}
}
