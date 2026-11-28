//! # Bucket rescans
//!
//! A bucket sends a client no change events, so a bucket source is listed
//! again on a timer and soon after an agent changes it. Each listing is a
//! full walk of the source: the sweep at its end drops the records of
//! objects that are gone, which a listing of one prefix could not prove.

use std::{
	collections::HashSet,
	path::{Path, PathBuf},
	sync::{Arc, LazyLock, Mutex, Weak},
	time::Duration,
};

use crate::{library::Library, ops::grants::bucket::BucketUrl};

/// How often every bucket source of a library is listed again.
pub const BUCKET_RESCAN_INTERVAL: Duration = Duration::from_secs(15 * 60);

/// How long a change waits for the changes behind it, so an agent's burst of
/// writes costs one listing.
const SETTLE: Duration = Duration::from_secs(2);

/// Roots with a listing already waiting to start.
static WAITING: LazyLock<Mutex<HashSet<PathBuf>>> = LazyLock::new(Default::default);

/// List every bucket source of `library` again on [`BUCKET_RESCAN_INTERVAL`],
/// until the library closes.
pub async fn rescan_buckets(library: Weak<Library>) {
	loop {
		tokio::time::sleep(BUCKET_RESCAN_INTERVAL).await;
		let Some(library) = library.upgrade() else {
			return;
		};
		let index = library.core_context().volume_index();
		for source in index.sources_of(library.id()) {
			if BucketUrl::parse(&source.root).is_some() && !index.is_indexing(&source.root) {
				rescan(&library, source.id, source.root).await;
			}
		}
	}
}

/// List the bucket source holding `path` again once it settles. Nothing
/// happens for a path in no bucket source of `library`.
pub fn rescan_soon(library: Arc<Library>, path: &Path) {
	let Some((id, root)) = library
		.core_context()
		.volume_index()
		.sources_of(library.id())
		.into_iter()
		.filter(|source| BucketUrl::parse(&source.root).is_some() && path.starts_with(&source.root))
		.max_by_key(|source| source.root.components().count())
		.map(|source| (source.id, source.root))
	else {
		return;
	};
	if !WAITING.lock().expect("rescan set").insert(root.clone()) {
		return;
	}
	tokio::spawn(async move {
		tokio::time::sleep(SETTLE).await;
		// A walk already running may have listed the prefix before the
		// change landed, so this one waits for it rather than being dropped.
		let index = library.core_context().volume_index();
		while index.is_indexing(&root) {
			tokio::time::sleep(SETTLE).await;
		}
		WAITING.lock().expect("rescan set").remove(&root);
		rescan(&library, id, root).await;
	});
}

async fn rescan(library: &Arc<Library>, id: uuid::Uuid, root: PathBuf) {
	let whole_volume = BucketUrl::parse(&root).is_some_and(|url| url.key.is_empty());
	let context = library.core_context().clone();
	if let Err(error) = crate::ops::sources::track::action::dispatch_source_walk(
		library,
		&context,
		id,
		root,
		whole_volume,
		false,
	)
	.await
	{
		tracing::warn!(source = %id, %error, "could not list the bucket source again");
	}
}
