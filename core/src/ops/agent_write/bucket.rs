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
//! Writes carry no undo; the provider's own object versioning is what
//! restores an earlier version. Moves and deletes are copies and deletes
//! that never lose a version, journaled so `files.undo` reverses them;
//! [`crate::ops::files::bucket`] has how, and where a service leaves a gap.

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
use uuid::Uuid;

use super::{
	conflict::conflict_names,
	ops::{journal, WriteTurn},
	state::{check, stale},
	PathKind, PathState, Precondition, WriteError, WriteOutcome, MAX_WRITE_BYTES,
};
use crate::{
	context::CoreContext,
	infra::job::journal::{Effect, Subject},
	ops::files::bucket::{self as files_bucket, Refusal},
	ops::grants::{
		bucket::{self, BucketUrl},
		GrantError, GrantScope,
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
		let backend = bucket::backend(&context.volume_manager, &url).await?;
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
	if listing.is_empty()
		&& !files_bucket::is_directory(bucket.backend.as_ref(), url)
			.await
			.map_err(io)?
	{
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

/// Objects a prefix move or delete takes before it is refused as too large
/// for one call.
const MAX_PREFIX_OBJECTS: usize = 10_000;

/// Move an object, or every object under a prefix, within one bucket.
///
/// Each object is copied with `If-None-Match` and its source deleted while
/// it still carries the tag it was copied at. A taken destination is kept and
/// the item lands under a conflict name beside it.
pub(super) async fn move_path(
	context: &CoreContext,
	library: Uuid,
	grant: Uuid,
	from: Bucket,
	to: Bucket,
	expect: Precondition,
	turn: &WriteTurn,
) -> Result<WriteOutcome, WriteError> {
	if from.url.volume() != to.url.volume() {
		return Err(WriteError::Invalid(
			"agents move within one volume; this move crosses volumes".to_string(),
		));
	}
	if from.url.contains(&to.url) {
		return Err(WriteError::Invalid(format!(
			"{} cannot move inside itself",
			from.shown(&from.url)
		)));
	}
	let current = object(&from, &from.url, turn.cancel()).await?;
	check(&expect, &current.state)?;
	recoverable(&from, "moving").await?;
	let is_dir = match current.state.kind {
		Some(PathKind::File) => false,
		Some(PathKind::Directory) => true,
		_ => return Err(stale(current.state)),
	};
	let id = Uuid::now_v7();
	let backend = from.backend.as_ref();
	let wanted = to.url.path();
	let candidates = std::iter::once(wanted.clone())
		.chain(conflict_names(&wanted, is_dir))
		.take(CONFLICT_ATTEMPTS);

	if !is_dir {
		let Some(source) = files_bucket::object(backend, &from.url.key)
			.await
			.map_err(io)?
			.filter(|source| Some(&source.etag) == current.etag.as_ref())
		else {
			return Err(stale(state(&from, turn.cancel()).await?));
		};
		for candidate in candidates {
			turn.proceed()?;
			let Some(url) = BucketUrl::parse(&candidate) else {
				continue;
			};
			if files_bucket::occupied(backend, &url).await.map_err(io)? {
				continue;
			}
			match files_bucket::relocate(backend, &source, &url.key).await {
				Ok(copy) => {
					let effects = vec![Effect::Moved {
						from: from.url.path(),
						to: url.path(),
						subject: Some(copy.subject()),
					}];
					let undo = journal(context, library, grant, id, &effects).await;
					return moved(&to, url, undo, turn).await;
				}
				Err(Refusal::Taken) => continue,
				Err(Refusal::Changed) => return Err(stale(state(&from, turn.cancel()).await?)),
				Err(Refusal::Kept) => {
					let effects = vec![Effect::Created {
						path: url.path(),
						subject: landed(backend, &url).await,
					}];
					let undo = journal(context, library, grant, id, &effects).await;
					return Err(kept(&from, &url, undo));
				}
				Err(refusal) => return Err(failed(&from, refusal)),
			}
		}
		return Err(WriteError::Io(format!(
			"no free name beside {} after {CONFLICT_ATTEMPTS} tries",
			to.shown(&to.url)
		)));
	}

	let sources = prefix_objects(&from, turn).await?;
	let mut target = None;
	for candidate in candidates {
		turn.proceed()?;
		let Some(url) = BucketUrl::parse(&candidate) else {
			continue;
		};
		if !files_bucket::occupied(backend, &url).await.map_err(io)? {
			target = Some(url);
			break;
		}
	}
	let Some(target) = target else {
		return Err(WriteError::Io(format!(
			"no free name beside {} after {CONFLICT_ATTEMPTS} tries",
			to.shown(&to.url)
		)));
	};
	if state(&from, turn.cancel()).await?.content_hash != current.state.content_hash {
		return Err(stale(state(&from, turn.cancel()).await?));
	}
	turn.proceed()?;
	let mut effects = Vec::new();
	let mut left = Vec::new();
	for source in &sources {
		let relative = source.key[from.url.key.len()..].trim_start_matches('/');
		let destination = target.join(relative);
		match files_bucket::relocate(backend, source, &destination.key).await {
			Ok(copy) => effects.push(Effect::Moved {
				from: files_bucket::path(&from.url, &source.key),
				to: destination.path(),
				subject: Some(copy.subject()),
			}),
			Err(refusal) => {
				if matches!(refusal, Refusal::Kept) {
					effects.push(Effect::Created {
						path: destination.path(),
						subject: landed(backend, &destination).await,
					});
				}
				left.push(format!(
					"{}: {refusal}",
					from.shown(&from.url.join(relative))
				));
			}
		}
	}
	let undo = journal(context, library, grant, id, &effects).await;
	if !left.is_empty() {
		return Err(partial("moving", &from, left, undo));
	}
	moved(&to, target, undo, turn).await
}

/// Put an object, or every object under a prefix, in the bucket's trash.
pub(super) async fn delete(
	context: &CoreContext,
	library: Uuid,
	grant: Uuid,
	bucket: Bucket,
	expect: Precondition,
	turn: &WriteTurn,
) -> Result<WriteOutcome, WriteError> {
	let current = object(&bucket, &bucket.url, turn.cancel()).await?;
	check(&expect, &current.state)?;
	recoverable(&bucket, "deleting").await?;
	let backend = bucket.backend.as_ref();
	let id = Uuid::now_v7();
	match current.state.kind {
		Some(PathKind::File) => {}
		Some(PathKind::Directory) => {
			return delete_prefix(context, library, grant, bucket, current.state, turn).await
		}
		_ => return Err(stale(current.state)),
	}
	{
		let Some(source) = files_bucket::object(backend, &bucket.url.key)
			.await
			.map_err(io)?
			.filter(|source| Some(&source.etag) == current.etag.as_ref())
		else {
			return Err(stale(state(&bucket, turn.cancel()).await?));
		};
		turn.proceed()?;
		return match files_bucket::trash(backend, &source, id).await {
			Ok(copy) => {
				let effects = vec![trashed(&bucket.url, &source, &copy)];
				let undo = journal(context, library, grant, id, &effects).await;
				deleted(&bucket, current.state, undo)
			}
			Err(Refusal::Changed) => Err(stale(state(&bucket, turn.cancel()).await?)),
			Err(Refusal::Kept) => Err(WriteError::Partial {
				message: format!(
					"stale: {} changed while it was being deleted, so it is left as it is, and a copy is kept in the bucket at {}",
					current.state.path,
					files_bucket::trash_key(id, &source.key)
				),
				undo: None,
			}),
			Err(refusal) => Err(failed(&bucket, refusal)),
		};
	}
}

/// Put every object under a prefix in the trash, each at the version the
/// snapshot taken with the precondition check found. An object replaced
/// since is left and named.
async fn delete_prefix(
	context: &CoreContext,
	library: Uuid,
	grant: Uuid,
	bucket: Bucket,
	checked: PathState,
	turn: &WriteTurn,
) -> Result<WriteOutcome, WriteError> {
	let backend = bucket.backend.as_ref();
	let id = Uuid::now_v7();
	let sources = prefix_objects(&bucket, turn).await?;
	if state(&bucket, turn.cancel()).await?.content_hash != checked.content_hash {
		return Err(stale(state(&bucket, turn.cancel()).await?));
	}
	turn.proceed()?;
	let mut effects = Vec::new();
	let mut left = Vec::new();
	for source in &sources {
		match files_bucket::trash(backend, source, id).await {
			Ok(copy) => effects.push(trashed(&bucket.url, source, &copy)),
			Err(refusal) => left.push(format!(
				"{}: {refusal}",
				bucket.shown(&files_bucket_url(&bucket.url, &source.key))
			)),
		}
	}
	let undo = journal(context, library, grant, id, &effects).await;
	if !left.is_empty() {
		return Err(partial("deleting", &bucket, left, undo));
	}
	deleted(&bucket, checked, undo)
}

/// Refuse a move or delete on a bucket where a write landing between the
/// tag check and the delete could be lost for good.
async fn recoverable(bucket: &Bucket, verb: &str) -> Result<(), WriteError> {
	match bucket.backend.deletes_are_recoverable().await {
		Ok(true) => Ok(()),
		Ok(false) => Err(WriteError::Invalid(format!(
			"{verb} objects needs object versioning on {}, so that a write landing while the object is copied is never lost; enable versioning on the bucket and try again",
			bucket.url.volume()
		))),
		Err(error) => Err(WriteError::Io(format!(
			"could not check versioning on {}: {error}",
			bucket.url.volume()
		))),
	}
}

fn files_bucket_url(url: &BucketUrl, key: &str) -> BucketUrl {
	BucketUrl {
		key: key.to_string(),
		..url.clone()
	}
}

/// Every object under the scope's prefix with the entity tag it has now,
/// refusing a prefix that holds a path the grant protects or more objects
/// than one call takes. Each object is then moved or trashed at that tag
/// only, so one written after the check is left alone.
async fn prefix_objects(
	bucket: &Bucket,
	turn: &WriteTurn,
) -> Result<Vec<files_bucket::Object>, WriteError> {
	turn.proceed()?;
	let keys =
		files_bucket::objects_under(bucket.backend.as_ref(), &bucket.url, MAX_PREFIX_OBJECTS)
			.await
			.map_err(io)?
			.ok_or_else(|| {
				WriteError::Invalid(format!(
					"{} holds more than {MAX_PREFIX_OBJECTS} objects; move or delete it in parts",
					bucket.shown(&bucket.url)
				))
			})?;
	for key in &keys {
		let relative = key[bucket.url.key.len()..].trim_start_matches('/');
		if crate::ops::grants::scope::protected_component(Path::new(relative)).is_some() {
			return Err(
				GrantError::Protected(bucket.shown(&files_bucket_url(&bucket.url, key))).into(),
			);
		}
	}
	let mut objects = Vec::with_capacity(keys.len());
	for key in &keys {
		turn.proceed()?;
		if let Some(object) = files_bucket::object(bucket.backend.as_ref(), key)
			.await
			.map_err(io)?
		{
			objects.push(object);
		}
	}
	Ok(objects)
}

async fn landed(backend: &dyn VolumeBackend, url: &BucketUrl) -> Option<Subject> {
	files_bucket::object(backend, &url.key)
		.await
		.ok()
		.flatten()
		.map(|object| object.subject())
}

fn trashed(url: &BucketUrl, source: &files_bucket::Object, copy: &files_bucket::Object) -> Effect {
	Effect::Trashed {
		from: files_bucket::path(url, &source.key),
		to: Some(files_bucket::path(url, &copy.key)),
		subject: Some(copy.subject()),
	}
}

fn kept(from: &Bucket, copy: &BucketUrl, undo: Option<Uuid>) -> WriteError {
	WriteError::Partial {
		message: format!(
			"stale: {} changed while it was being moved, so it is left as it is and the copy at {} is kept",
			from.shown(&from.url),
			from.shown(copy)
		),
		undo,
	}
}

fn partial(verb: &str, bucket: &Bucket, left: Vec<String>, undo: Option<Uuid>) -> WriteError {
	WriteError::Partial {
		message: format!(
			"{verb} {} left {} object(s) as they are: {}",
			bucket.shown(&bucket.url),
			left.len(),
			left.join("; ")
		),
		undo,
	}
}

fn failed(bucket: &Bucket, refusal: Refusal) -> WriteError {
	match refusal {
		Refusal::TooLarge(_) => {
			WriteError::Invalid(format!("{}: {refusal}", bucket.shown(&bucket.url)))
		}
		refusal => WriteError::Io(format!("{}: {refusal}", bucket.shown(&bucket.url))),
	}
}

async fn moved(
	to: &Bucket,
	landed: BucketUrl,
	undo: Option<Uuid>,
	turn: &WriteTurn,
) -> Result<WriteOutcome, WriteError> {
	let mut outcome = outcome(to, to.shown(&to.url), landed, turn).await?;
	outcome.undo = undo;
	Ok(outcome)
}

fn deleted(
	bucket: &Bucket,
	before: PathState,
	undo: Option<Uuid>,
) -> Result<WriteOutcome, WriteError> {
	info!(bucket = %bucket.url.volume(), path = %before.path, "agent delete in a bucket");
	Ok(WriteOutcome {
		requested: bucket.shown(&bucket.url),
		state: PathState {
			kind: None,
			size: None,
			content_hash: None,
			..before
		},
		conflict: false,
		undo,
	})
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
