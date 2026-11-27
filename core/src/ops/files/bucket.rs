//! # Moving and trashing bucket objects
//!
//! A bucket has no rename and no trash, so a move is a copy and a delete, and
//! a delete is a copy into a trash prefix in the same bucket and a delete.
//! Every copy names the version it copies by entity tag, and the source is
//! deleted only while it still carries that tag, so a version written by
//! anyone while the operation runs is kept.
//!
//! A copy to a key someone else may write, the destination of a move or the
//! original key on a restore, is sent with `If-None-Match: *` so it never
//! replaces anything. That copy passes the bytes through this device, so it
//! is held to [`MAX_COPY_BYTES`]. A copy into the trash goes to a key only
//! this operation names, so the service makes it at any size.
//!
//! The service is asked to refuse a changed source itself where it can.
//! S3 through OpenDAL cannot refuse a delete, so the tag is checked just
//! before an unconditional delete; a write that lands between that check and
//! the delete is lost from the key. The window is one round trip.

use std::{
	path::{Path, PathBuf},
	time::SystemTime,
};

use uuid::Uuid;

use crate::{
	infra::job::journal::Subject,
	ops::{grants::bucket::BucketUrl, indexing::state::EntryKind},
	volume::{VolumeBackend, VolumeError, WriteCondition},
};

/// The prefix at a bucket's root that holds what Spacedrive trashed there,
/// the same name a volume without a trash of its own gets.
pub const TRASH: &str = super::trash::SPACEDRIVE_TRASH;

/// The largest object copied through this device under `If-None-Match`.
pub const MAX_COPY_BYTES: u64 = 64 * 1024 * 1024;

/// One object as it is now.
#[derive(Debug, Clone)]
pub struct Object {
	pub key: String,
	pub etag: String,
	pub size: u64,
	pub modified: Option<SystemTime>,
}

impl Object {
	/// What a journal records about it, for undo to check against later.
	pub fn subject(&self) -> Subject {
		Subject {
			size: self.size,
			mtime_ms: self
				.modified
				.and_then(|at| at.duration_since(SystemTime::UNIX_EPOCH).ok())
				.map(|at| at.as_millis() as i64)
				.unwrap_or(0),
			is_dir: false,
			created_ms: None,
			etag: Some(self.etag.clone()),
		}
	}
}

/// Why a copy, move or trashing stopped.
#[derive(Debug)]
pub enum Refusal {
	/// The object changed or went before anything was copied.
	Changed,
	/// Something already sits at the destination.
	Taken,
	/// The copy landed, then the object changed before it could be deleted,
	/// so both are kept.
	Kept,
	/// The object is larger than a conditional copy takes.
	TooLarge(u64),
	Failed(VolumeError),
}

impl std::fmt::Display for Refusal {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Changed => write!(f, "the object changed before it was copied"),
			Self::Taken => write!(f, "something already sits at the destination"),
			Self::Kept => write!(
				f,
				"the object changed after it was copied; both the copy and the object are kept"
			),
			Self::TooLarge(size) => write!(
				f,
				"the object is {size} bytes, and a copy that must not replace anything takes up to {} MiB",
				MAX_COPY_BYTES / (1024 * 1024)
			),
			Self::Failed(error) => write!(f, "{error}"),
		}
	}
}

fn refused(error: VolumeError, refusal: Refusal) -> Refusal {
	match error {
		VolumeError::PreconditionFailed(_) => refusal,
		error => Refusal::Failed(error),
	}
}

/// The object at `key`, or `None` when no object is there.
pub async fn object(backend: &dyn VolumeBackend, key: &str) -> Result<Option<Object>, VolumeError> {
	let Some(etag) = backend.entity_tag(Path::new(key)).await? else {
		return Ok(None);
	};
	let meta = match backend.metadata(Path::new(key)).await {
		Ok(meta) if meta.kind != EntryKind::Directory => meta,
		Ok(_) => return Ok(None),
		Err(VolumeError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
			return Ok(None)
		}
		Err(error) => return Err(error),
	};
	Ok(Some(Object {
		key: key.to_string(),
		etag,
		size: meta.size,
		modified: meta.modified,
	}))
}

/// Whether anything sits at `key`: an object, or objects below it.
pub async fn occupied(backend: &dyn VolumeBackend, url: &BucketUrl) -> Result<bool, VolumeError> {
	if backend.entity_tag(Path::new(&url.key)).await?.is_some() {
		return Ok(true);
	}
	Ok(!crate::ops::grants::bucket::list(backend, url)
		.await?
		.is_empty())
}

/// Every object below the prefix `url` names, by key, in key order.
/// Directory markers are left out; they hold no bytes.
pub async fn objects_under(
	backend: &dyn VolumeBackend,
	url: &BucketUrl,
	limit: usize,
) -> Result<Option<Vec<String>>, VolumeError> {
	let mut found = Vec::new();
	let mut pending = vec![url.clone()];
	while let Some(prefix) = pending.pop() {
		for entry in crate::ops::grants::bucket::list(backend, &prefix).await? {
			let child = prefix.join(&entry.name);
			if entry.kind == EntryKind::Directory {
				pending.push(child);
			} else {
				found.push(child.key);
				if found.len() > limit {
					return Ok(None);
				}
			}
		}
	}
	found.sort();
	Ok(Some(found))
}

/// Copy `object` to `to` only while it carries its tag and only if nothing
/// is at `to`.
pub async fn copy_new(
	backend: &dyn VolumeBackend,
	object: &Object,
	to: &str,
) -> Result<(), Refusal> {
	if object.size > MAX_COPY_BYTES {
		return Err(Refusal::TooLarge(object.size));
	}
	let bytes = backend
		.read_if_match(Path::new(&object.key), &object.etag)
		.await
		.map_err(|error| refused(error, Refusal::Changed))?;
	backend
		.write_if(Path::new(to), bytes, WriteCondition::Absent)
		.await
		.map_err(|error| refused(error, Refusal::Taken))?;
	Ok(())
}

/// Move `object` to `to`: a copy that replaces nothing, then a delete of
/// the source while it still carries its tag. Answers with the copy, read
/// before the delete so a failure after it cannot lose the journal record.
pub async fn relocate(
	backend: &dyn VolumeBackend,
	object: &Object,
	to: &str,
) -> Result<Object, Refusal> {
	copy_new(backend, object, to).await?;
	let copy = self::object(backend, to)
		.await
		.map_err(Refusal::Failed)?
		.ok_or_else(|| {
			Refusal::Failed(VolumeError::Io(std::io::Error::other(format!(
				"the copy at {to} is missing"
			))))
		})?;
	backend
		.delete_if_match(Path::new(&object.key), &object.etag)
		.await
		.map_err(|error| refused(error, Refusal::Kept))?;
	Ok(copy)
}

/// The trash key an object goes to for one job.
pub fn trash_key(job: Uuid, key: &str) -> String {
	format!("{TRASH}/{job}/{key}")
}

/// Put `object` in the bucket's trash and answer with the trash copy.
///
/// The copy is made inside the service, then the source's tag is checked
/// again so the copy is known to hold the version that is deleted. A source
/// that changed meanwhile is kept, and so is the copy. An object larger than
/// a restore can copy back under `If-None-Match` is refused, so whatever
/// goes to the trash can come back.
pub async fn trash(
	backend: &dyn VolumeBackend,
	object: &Object,
	job: Uuid,
) -> Result<Object, Refusal> {
	if object.size > MAX_COPY_BYTES {
		return Err(Refusal::TooLarge(object.size));
	}
	let to = trash_key(job, &object.key);
	backend
		.copy_within(Path::new(&object.key), Path::new(&to))
		.await
		.map_err(|error| refused(error, Refusal::Changed))?;
	match backend.entity_tag(Path::new(&object.key)).await {
		Ok(Some(etag)) if etag == object.etag => {}
		Ok(_) => return Err(Refusal::Kept),
		Err(error) => return Err(Refusal::Failed(error)),
	}
	// Read the copy before the delete, so nothing can fail between the delete
	// and the journal that records where the bytes went.
	let copy = self::object(backend, &to)
		.await
		.map_err(Refusal::Failed)?
		.ok_or_else(|| {
			Refusal::Failed(VolumeError::Io(std::io::Error::other(format!(
				"the trash copy at {to} is missing"
			))))
		})?;
	backend
		.delete_if_match(Path::new(&object.key), &object.etag)
		.await
		.map_err(|error| refused(error, Refusal::Kept))?;
	Ok(copy)
}

/// The object at `key` while it is still the one a journal recorded: the
/// same entity tag, or for a row without one the same size and time.
pub async fn still_holds(
	backend: &dyn VolumeBackend,
	key: &str,
	subject: &Subject,
) -> Option<Object> {
	let now = object(backend, key).await.ok().flatten()?;
	let holds = match &subject.etag {
		Some(etag) => &now.etag == etag,
		None => {
			let recorded = now.subject();
			recorded.size == subject.size && recorded.mtime_ms == subject.mtime_ms
		}
	};
	holds.then_some(now)
}

/// A journal path for a key in the bucket `url` names.
pub fn path(url: &BucketUrl, key: &str) -> PathBuf {
	BucketUrl {
		key: key.to_string(),
		..url.clone()
	}
	.path()
}

#[cfg(test)]
mod tests {
	use std::{collections::HashMap, ops::Range, sync::Mutex};

	use async_trait::async_trait;
	use bytes::Bytes;

	use super::*;
	use crate::volume::{
		backend::{BackendType, CloudServiceType},
		RawDirEntry, RawMetadata,
	};

	/// A bucket in memory where another writer changes the source the moment
	/// a copy of it lands, the gap a service without conditional deletes
	/// leaves open.
	#[derive(Debug, Default)]
	struct Racing {
		objects: Mutex<HashMap<String, (Bytes, u32)>>,
		race: bool,
	}

	impl Racing {
		fn with(objects: &[(&str, &str)], race: bool) -> Self {
			Self {
				objects: Mutex::new(
					objects
						.iter()
						.map(|(key, body)| (key.to_string(), (Bytes::from(body.to_string()), 1)))
						.collect(),
				),
				race,
			}
		}

		fn body(&self, key: &str) -> Option<String> {
			self.objects
				.lock()
				.unwrap()
				.get(key)
				.map(|(bytes, _)| String::from_utf8(bytes.to_vec()).unwrap())
		}

		fn copied(&self, from: &Path) {
			if !self.race {
				return;
			}
			let mut objects = self.objects.lock().unwrap();
			if let Some((bytes, version)) = objects.get_mut(from.to_str().unwrap()) {
				*bytes = Bytes::from("changed elsewhere");
				*version += 1;
			}
		}
	}

	fn key(path: &Path) -> String {
		path.to_str().unwrap().to_string()
	}

	#[async_trait]
	impl VolumeBackend for Racing {
		async fn read(&self, path: &Path) -> Result<Bytes, VolumeError> {
			self.objects
				.lock()
				.unwrap()
				.get(&key(path))
				.map(|(bytes, _)| bytes.clone())
				.ok_or_else(|| VolumeError::Io(std::io::ErrorKind::NotFound.into()))
		}
		async fn read_range(&self, _: &Path, _: Range<u64>) -> Result<Bytes, VolumeError> {
			unreachable!()
		}
		async fn write(&self, _: &Path, _: Bytes) -> Result<(), VolumeError> {
			unreachable!()
		}
		async fn read_dir(&self, _: &Path) -> Result<Vec<RawDirEntry>, VolumeError> {
			Ok(Vec::new())
		}
		async fn metadata(&self, path: &Path) -> Result<RawMetadata, VolumeError> {
			let size = self.read(path).await?.len() as u64;
			Ok(RawMetadata {
				kind: EntryKind::File,
				size,
				modified: None,
				created: None,
				accessed: None,
				inode: None,
				permissions: None,
				uid: None,
				gid: None,
				link_target: None,
			})
		}
		async fn exists(&self, path: &Path) -> Result<bool, VolumeError> {
			Ok(self.objects.lock().unwrap().contains_key(&key(path)))
		}
		async fn delete(&self, _: &Path) -> Result<(), VolumeError> {
			unreachable!()
		}
		async fn create_directory(&self, _: &Path, _: bool) -> Result<(), VolumeError> {
			unreachable!()
		}
		async fn write_if(
			&self,
			path: &Path,
			data: Bytes,
			condition: WriteCondition,
		) -> Result<Option<String>, VolumeError> {
			assert_eq!(condition, WriteCondition::Absent);
			{
				let mut objects = self.objects.lock().unwrap();
				if objects.contains_key(&key(path)) {
					return Err(VolumeError::PreconditionFailed("taken".into()));
				}
				objects.insert(key(path), (data, 1));
			}
			Ok(None)
		}
		async fn entity_tag(&self, path: &Path) -> Result<Option<String>, VolumeError> {
			Ok(self
				.objects
				.lock()
				.unwrap()
				.get(&key(path))
				.map(|(_, version)| version.to_string()))
		}
		async fn read_if_match(&self, path: &Path, etag: &str) -> Result<Bytes, VolumeError> {
			if self.entity_tag(path).await?.as_deref() != Some(etag) {
				return Err(VolumeError::PreconditionFailed("changed".into()));
			}
			let bytes = self.read(path).await?;
			self.copied(path);
			Ok(bytes)
		}
		async fn delete_if_match(&self, path: &Path, etag: &str) -> Result<(), VolumeError> {
			if self.entity_tag(path).await?.as_deref() != Some(etag) {
				return Err(VolumeError::PreconditionFailed("changed".into()));
			}
			self.objects.lock().unwrap().remove(&key(path));
			Ok(())
		}
		async fn copy_within(&self, from: &Path, to: &Path) -> Result<(), VolumeError> {
			let bytes = self.read(from).await?;
			self.objects.lock().unwrap().insert(key(to), (bytes, 1));
			self.copied(from);
			Ok(())
		}
		fn is_local(&self) -> bool {
			false
		}
		fn backend_type(&self) -> BackendType {
			BackendType::Cloud(CloudServiceType::S3)
		}
	}

	async fn at(backend: &Racing, key: &str) -> Object {
		object(backend, key).await.unwrap().expect("an object")
	}

	#[tokio::test]
	async fn a_move_lands_and_removes_the_source() {
		let bucket = Racing::with(&[("a.txt", "alpha")], false);
		let source = at(&bucket, "a.txt").await;
		relocate(&bucket, &source, "b.txt").await.unwrap();
		assert_eq!(bucket.body("b.txt").as_deref(), Some("alpha"));
		assert_eq!(bucket.body("a.txt"), None);
	}

	#[tokio::test]
	async fn a_move_never_replaces_the_destination() {
		let bucket = Racing::with(&[("a.txt", "alpha"), ("b.txt", "beta")], false);
		let source = at(&bucket, "a.txt").await;
		let refused = relocate(&bucket, &source, "b.txt").await;
		assert!(matches!(refused, Err(Refusal::Taken)), "{refused:?}");
		assert_eq!(bucket.body("a.txt").as_deref(), Some("alpha"));
		assert_eq!(bucket.body("b.txt").as_deref(), Some("beta"));
	}

	#[tokio::test]
	async fn a_move_keeps_both_when_the_source_changes_after_the_copy() {
		let bucket = Racing::with(&[("a.txt", "alpha")], true);
		let source = at(&bucket, "a.txt").await;
		let refused = relocate(&bucket, &source, "b.txt").await;
		assert!(matches!(refused, Err(Refusal::Kept)), "{refused:?}");
		assert_eq!(bucket.body("a.txt").as_deref(), Some("changed elsewhere"));
		assert_eq!(bucket.body("b.txt").as_deref(), Some("alpha"));
	}

	#[tokio::test]
	async fn a_trashing_keeps_a_source_that_changed_after_the_copy() {
		let bucket = Racing::with(&[("a.txt", "alpha")], true);
		let source = at(&bucket, "a.txt").await;
		let job = Uuid::now_v7();
		let refused = trash(&bucket, &source, job).await;
		assert!(matches!(refused, Err(Refusal::Kept)), "{refused:?}");
		assert_eq!(bucket.body("a.txt").as_deref(), Some("changed elsewhere"));
		assert_eq!(
			bucket.body(&trash_key(job, "a.txt")).as_deref(),
			Some("alpha")
		);
	}

	#[tokio::test]
	async fn a_trashing_keeps_the_version_it_deletes() {
		let bucket = Racing::with(&[("dir/a.txt", "alpha")], false);
		let source = at(&bucket, "dir/a.txt").await;
		let job = Uuid::now_v7();
		let copy = trash(&bucket, &source, job).await.unwrap();
		assert_eq!(copy.key, format!(".spacedrive/trash/{job}/dir/a.txt"));
		assert_eq!(bucket.body("dir/a.txt"), None);
		assert_eq!(bucket.body(&copy.key).as_deref(), Some("alpha"));
	}
}
