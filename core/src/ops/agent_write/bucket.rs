//! Agent writes to a bucket source.
//!
//! A bucket refuses a stale write itself: every replacement is sent with the
//! entity tag of the version the precondition was checked against, as
//! `If-Match`, and every create with `If-None-Match: *`. A change that lands
//! between the check and the write makes the service refuse it, so no lock
//! or second check is needed, and a write from outside Spacedrive is caught
//! the same way as one from another agent.
//!
//! A precondition names what it names on disk: the BLAKE3 hash of the
//! object's bytes, read at the entity tag that is then sent with the write.
//! Buckets have no store revision here, so a revision precondition is always
//! stale. A create that finds the key taken lands under the next free
//! `name (agent conflict N).ext`, decided by the service refusing each taken
//! name in turn, so two creates racing for one name both keep their bytes.
//!
//! Buckets keep no trash, so these writes carry no undo; the provider's own
//! object versioning is what restores an earlier version. Moves and deletes
//! are refused until the service can make them conditional too.

use std::{
	path::Path,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
};

use bytes::Bytes;
use futures::StreamExt;
use tracing::info;

use super::{
	conflict::conflict_names,
	ops::WriteTurn,
	state::{check, stale},
	PathKind, PathState, Precondition, WriteError, WriteOutcome, MAX_WRITE_BYTES,
};
use crate::{
	context::CoreContext,
	ops::grants::{
		bucket::{self, BucketUrl},
		GrantScope,
	},
	ops::indexing::state::EntryKind,
	volume::{VolumeBackend, VolumeError, WriteCondition},
};

/// How many times a read is retried when the object changes between its
/// tag and its bytes.
const READ_ATTEMPTS: usize = 3;
/// Conflict names tried before a create gives up.
const CONFLICT_ATTEMPTS: usize = 1000;

/// The bucket behind an authorized scope.
pub(super) struct Bucket {
	root: BucketUrl,
	url: BucketUrl,
	backend: Arc<dyn VolumeBackend>,
}

impl Bucket {
	/// The bucket a scope names, or `None` for a filesystem scope.
	pub(super) async fn of(
		context: &CoreContext,
		scope: &GrantScope,
	) -> Result<Option<Self>, WriteError> {
		Self::at(context, &scope.root, &scope.path).await
	}

	pub(super) async fn at(
		context: &CoreContext,
		root: &Path,
		path: &Path,
	) -> Result<Option<Self>, WriteError> {
		let (Some(root), Some(url)) = (BucketUrl::parse(root), BucketUrl::parse(path)) else {
			return Ok(None);
		};
		let backend = bucket::backend(context, &url).await?;
		Ok(Some(Self { root, url, backend }))
	}

	fn shown(&self, url: &BucketUrl) -> String {
		url.key
			.strip_prefix(&self.root.key)
			.unwrap_or(&url.key)
			.trim_start_matches('/')
			.to_string()
	}
}

/// A key as it is now: its state, and for a file the tag the hash was taken
/// from.
struct Object {
	state: PathState,
	etag: Option<String>,
}

fn io(error: VolumeError) -> WriteError {
	WriteError::Io(error.to_string())
}

/// Read the state of `url` in `bucket`.
async fn object(
	bucket: &Bucket,
	url: &BucketUrl,
	cancel: &AtomicBool,
) -> Result<Object, WriteError> {
	let path = bucket.shown(url);
	let absent = |kind, size, hash| PathState {
		path: path.clone(),
		kind,
		size,
		content_hash: hash,
		revision: None,
	};
	if !url.key.is_empty() {
		for _ in 0..READ_ATTEMPTS {
			if cancel.load(Ordering::Relaxed) {
				return Err(WriteError::Cancelled);
			}
			let Some(etag) = bucket
				.backend
				.entity_tag(Path::new(&url.key))
				.await
				.map_err(io)?
			else {
				break;
			};
			match hash_at(bucket, url, &etag, cancel).await {
				Ok((hash, size)) => {
					return Ok(Object {
						state: absent(Some(PathKind::File), Some(size), Some(hash)),
						etag: Some(etag),
					});
				}
				Err(_) if cancel.load(Ordering::Relaxed) => return Err(WriteError::Cancelled),
				Err(VolumeError::PreconditionFailed(_)) => continue,
				Err(error) => return Err(io(error)),
			}
		}
	}
	let listing = list(bucket, url).await?;
	if listing.is_empty() && !url.key.is_empty() {
		return Ok(Object {
			state: absent(None, None, None),
			etag: None,
		});
	}
	Ok(Object {
		state: absent(
			Some(PathKind::Directory),
			Some(0),
			Some(listing_hash(&listing)),
		),
		etag: None,
	})
}

/// Hash the version `etag` names as it streams in, so an object of any size
/// costs one chunk of memory. A cancelled call stops between chunks.
async fn hash_at(
	bucket: &Bucket,
	url: &BucketUrl,
	etag: &str,
	cancel: &AtomicBool,
) -> Result<(String, u64), VolumeError> {
	let mut stream = bucket
		.backend
		.stream_if_match(Path::new(&url.key), etag)
		.await?;
	let mut hasher = blake3::Hasher::new();
	let mut size = 0u64;
	while let Some(chunk) = stream.next().await {
		if cancel.load(Ordering::Relaxed) {
			return Err(VolumeError::Io(std::io::Error::new(
				std::io::ErrorKind::Interrupted,
				"cancelled",
			)));
		}
		let chunk = chunk?;
		size += chunk.len() as u64;
		hasher.update(&chunk);
	}
	Ok((hasher.finalize().to_hex().to_string(), size))
}

async fn list(
	bucket: &Bucket,
	url: &BucketUrl,
) -> Result<Vec<crate::volume::RawDirEntry>, WriteError> {
	bucket::list(bucket.backend.as_ref(), url).await.map_err(io)
}

/// The same listing hash a local directory gets: each child's name, kind,
/// size and modification time.
fn listing_hash(entries: &[crate::volume::RawDirEntry]) -> String {
	let mut hasher = blake3::Hasher::new();
	for entry in entries {
		let modified = entry
			.modified
			.and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
			.map(|at| at.as_nanos())
			.unwrap_or(0);
		hasher.update(entry.name.as_bytes());
		hasher.update(b"\0");
		hasher.update(
			format!(
				"{}:{}:{}",
				if entry.kind == EntryKind::Directory {
					"d"
				} else {
					"f"
				},
				if entry.kind == EntryKind::Directory {
					0
				} else {
					entry.size
				},
				modified
			)
			.as_bytes(),
		);
		hasher.update(b"\n");
	}
	hasher.finalize().to_hex().to_string()
}

/// The state of a key a grant check already resolved.
pub(super) async fn state(bucket: &Bucket, cancel: &AtomicBool) -> Result<PathState, WriteError> {
	Ok(object(bucket, &bucket.url, cancel).await?.state)
}

/// Write a whole object, or a byte range of one, under the scope's key.
pub(super) async fn write_file(
	bucket: Bucket,
	bytes: Vec<u8>,
	offset: Option<u64>,
	expect: Option<Precondition>,
	turn: &WriteTurn,
) -> Result<WriteOutcome, WriteError> {
	let requested = bucket.shown(&bucket.url);
	let Some(expect) = expect else {
		let landed = create(&bucket, Bytes::from(bytes), turn).await?;
		return outcome(&bucket, requested, landed, turn).await;
	};

	let current = object(&bucket, &bucket.url, turn.cancel()).await?;
	check(&expect, &current.state)?;
	if current.state.kind != Some(PathKind::File) {
		return Err(WriteError::Invalid(format!(
			"{} is not a regular file",
			current.state.path
		)));
	}
	let Some(etag) = current.etag else {
		return Err(stale(current.state));
	};
	let body = match offset {
		None => Bytes::from(bytes),
		Some(offset) => {
			// A bucket has no partial write, so a range write sends the whole
			// object back and is held to the size of one write.
			let size = current.state.size.unwrap_or(0);
			if size > MAX_WRITE_BYTES as u64 {
				return Err(WriteError::Invalid(format!(
					"{} is {size} bytes; a byte-range write to a bucket object rewrites it whole and takes objects up to {} MiB, so write the whole object instead",
					current.state.path,
					MAX_WRITE_BYTES / (1024 * 1024)
				)));
			}
			let existing = match bucket
				.backend
				.read_if_match(Path::new(&bucket.url.key), &etag)
				.await
			{
				Ok(existing) => existing,
				Err(VolumeError::PreconditionFailed(_)) => {
					let now = object(&bucket, &bucket.url, turn.cancel()).await?;
					return Err(stale(now.state));
				}
				Err(error) => return Err(io(error)),
			};
			let offset = usize::try_from(offset)
				.ok()
				.filter(|offset| *offset <= existing.len())
				.ok_or_else(|| {
					WriteError::Invalid(format!(
						"offset {offset} is past the end of {} ({} bytes)",
						current.state.path,
						existing.len()
					))
				})?;
			let mut spliced = existing.to_vec();
			let end = offset + bytes.len();
			if end > spliced.len() {
				spliced.resize(end, 0);
			}
			spliced[offset..end].copy_from_slice(&bytes);
			Bytes::from(spliced)
		}
	};
	turn.proceed()?;
	match bucket
		.backend
		.write_if(
			Path::new(&bucket.url.key),
			body,
			WriteCondition::Matches(etag),
		)
		.await
	{
		Ok(_) => {}
		Err(VolumeError::PreconditionFailed(_)) => {
			let now = object(&bucket, &bucket.url, &AtomicBool::new(false)).await?;
			return Err(stale(now.state));
		}
		Err(error) => return Err(io(error)),
	}
	outcome(&bucket, requested, bucket.url.clone(), turn).await
}

/// Create the scope's key, or the first free conflict name beside it. The
/// service decides what is free by refusing each taken name.
async fn create(bucket: &Bucket, bytes: Bytes, turn: &WriteTurn) -> Result<BucketUrl, WriteError> {
	let wanted = bucket.url.path();
	let candidates = std::iter::once(wanted.clone())
		.chain(conflict_names(&wanted, false))
		.take(CONFLICT_ATTEMPTS);
	for candidate in candidates {
		turn.proceed()?;
		let Some(url) = BucketUrl::parse(&candidate) else {
			continue;
		};
		// A prefix with objects under it reads as a directory, and a file
		// over it would hide nothing but would confuse every listing.
		if !list(bucket, &url).await?.is_empty() {
			continue;
		}
		turn.proceed()?;
		match bucket
			.backend
			.write_if(Path::new(&url.key), bytes.clone(), WriteCondition::Absent)
			.await
		{
			Ok(_) => return Ok(url),
			Err(VolumeError::PreconditionFailed(_)) => continue,
			Err(error) => return Err(io(error)),
		}
	}
	Err(WriteError::Io(format!(
		"no free name beside {} after {CONFLICT_ATTEMPTS} tries",
		bucket.shown(&bucket.url)
	)))
}

/// Make a directory marker at the scope's key. An existing directory is the
/// answer; a file at the key keeps it and the directory takes a conflict
/// name.
pub(super) async fn make_dir(bucket: Bucket, turn: &WriteTurn) -> Result<WriteOutcome, WriteError> {
	let requested = bucket.shown(&bucket.url);
	let wanted = bucket.url.path();
	let candidates = std::iter::once(wanted.clone())
		.chain(conflict_names(&wanted, true))
		.take(CONFLICT_ATTEMPTS);
	for candidate in candidates {
		turn.proceed()?;
		let Some(url) = BucketUrl::parse(&candidate) else {
			continue;
		};
		let current = object(&bucket, &url, turn.cancel()).await?;
		match current.state.kind {
			Some(PathKind::Directory) if url == bucket.url => {
				return outcome(&bucket, requested, url, turn).await;
			}
			Some(_) => continue,
			None => {}
		}
		turn.proceed()?;
		bucket
			.backend
			.create_directory(Path::new(&url.key), true)
			.await
			.map_err(io)?;
		return outcome(&bucket, requested, url, turn).await;
	}
	Err(WriteError::Io(format!(
		"no free name beside {requested} after {CONFLICT_ATTEMPTS} tries"
	)))
}

/// Moves and deletes on a bucket would act on whatever is at the key when
/// they land, so they are refused rather than risk losing a newer version.
pub(super) fn unconditional(op: &str) -> WriteError {
	WriteError::Invalid(format!(
		"{op} is not available on bucket sources, since the bucket cannot refuse it against a changed object; write a new version instead"
	))
}

async fn outcome(
	bucket: &Bucket,
	requested: String,
	landed: BucketUrl,
	turn: &WriteTurn,
) -> Result<WriteOutcome, WriteError> {
	let conflict = landed != bucket.url;
	let state = object(bucket, &landed, turn.cancel()).await?.state;
	info!(bucket = %bucket.url.volume(), path = %state.path, conflict, "agent write to a bucket");
	Ok(WriteOutcome {
		requested,
		state,
		conflict,
		undo: None,
	})
}
