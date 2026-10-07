//! The source stores that hold what is beneath a path.
//!
//! A registered source's store holds the records beneath its root, except
//! beneath a source nested inside it: the innermost registered source keeps a
//! path's records, so a nested source's files live in its own store rather
//! than the outer one's. What is beneath a path is therefore the innermost
//! source holding the path, read from the path down, and every source nested
//! beneath the path, read whole.
//!
//! The sources this device replicates from a paired device nest the same way
//! beneath paths on that device. File operations never reach them, since a
//! replica's files change only on the device that owns them; viewers do.

use std::path::Path;
use std::sync::Arc;

use crate::context::CoreContext;
use crate::domain::SdPath;
use crate::ops::indexing::{volume_index::SourceStatus, VolumeIndex};
use crate::service::mounts::peer::{self, RemoteShare};
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
	innermost_root(sources.iter().map(|source| source.root.as_path()), path)
		.map(|index| &sources[index])
}

/// The position of the innermost of `roots` holding `path`. An empty root
/// holds nothing.
fn innermost_root<'a>(roots: impl Iterator<Item = &'a Path>, path: &Path) -> Option<usize> {
	roots
		.enumerate()
		.filter(|(_, root)| !root.as_os_str().is_empty() && path.starts_with(root))
		.max_by_key(|(_, root)| root.as_os_str().len())
		.map(|(index, _)| index)
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
	let roots: Vec<&Path> = sources.iter().map(|source| source.root.as_path()).collect();
	let mut reached = reach_roots(&roots, path).into_iter().peekable();
	sources
		.into_iter()
		.enumerate()
		.filter_map(|(index, source)| {
			let (_, scope, prefix) = reached.next_if(|(at, ..)| *at == index)?;
			Some(Reach {
				source,
				scope,
				prefix,
			})
		})
		.collect()
}

/// A source this device replicates from a paired device, reached by a path on
/// that device, and how its files sit beneath the path.
#[derive(Clone)]
pub struct ReplicaReach {
	pub share: Arc<RemoteShare>,
	/// The owning device's slug, which addresses the replica's files.
	pub device_slug: String,
	/// The directory the replica is read from, relative to its root.
	pub scope: String,
	/// Where the replica's files sit relative to the path.
	pub prefix: String,
}

/// The replicas beneath a path on another device, in root order. A path on
/// this device reaches none.
pub async fn replicas_beneath(context: &CoreContext, path: &SdPath) -> Vec<ReplicaReach> {
	if path.as_local_path().is_some() {
		return Vec::new();
	}
	let SdPath::Physical { device_slug, path } = path else {
		return Vec::new();
	};
	let shares = replicas_of(context, Some(device_slug)).await;
	let roots: Vec<&Path> = shares
		.iter()
		.map(|(share, _)| share.info.root.as_path())
		.collect();
	reach_roots(&roots, path)
		.into_iter()
		.map(|(index, scope, prefix)| ReplicaReach {
			share: shares[index].0.clone(),
			device_slug: shares[index].1.clone(),
			scope,
			prefix,
		})
		.collect()
}

/// Every replica this device holds, each read whole, by device and then root.
pub async fn every_replica(context: &CoreContext) -> Vec<ReplicaReach> {
	replicas_of(context, None)
		.await
		.into_iter()
		.map(|(share, device_slug)| ReplicaReach {
			share,
			device_slug,
			scope: String::new(),
			prefix: String::new(),
		})
		.collect()
}

/// Replicas with their owners' slugs, of the one device `device_slug` names
/// or of every device, ordered by owner and root. A replica whose owner has no
/// slug here cannot be addressed and is left out.
async fn replicas_of(
	context: &CoreContext,
	device_slug: Option<&str>,
) -> Vec<(Arc<RemoteShare>, String)> {
	let mut shares: Vec<(Arc<RemoteShare>, String)> = peer::remote_shares()
		.await
		.into_iter()
		.filter_map(|share| {
			let slug = context.device_manager.get_device_slug(share.device_id)?;
			device_slug
				.is_none_or(|wanted| wanted == slug)
				.then_some((share, slug))
		})
		.collect();
	shares.sort_by(|(a, a_slug), (b, b_slug)| {
		a_slug
			.cmp(b_slug)
			.then_with(|| a.info.root.cmp(&b.info.root))
			.then_with(|| a.info.id.cmp(&b.info.id))
	});
	shares
}

/// Which of `roots` a path reaches, as `(index, scope, prefix)` in the roots'
/// order: the innermost root holding the path, read from the path down, and
/// every root nested beneath the path, read whole. An empty root holds nothing.
fn reach_roots(roots: &[&Path], path: &Path) -> Vec<(usize, String, String)> {
	let innermost = innermost_root(roots.iter().copied(), path);
	roots
		.iter()
		.enumerate()
		.filter_map(|(index, root)| {
			if Some(index) == innermost {
				Some((index, relative(path, root)?, String::new()))
			} else if root.starts_with(path) {
				Some((index, String::new(), relative(root, path)?))
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
		library: None,
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
