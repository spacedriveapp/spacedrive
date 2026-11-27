//! Grants over a prefix of a bucket source.
//!
//! A bucket has no symlinks, firmlinks or case aliases, so a request is
//! resolved by its spelling alone: every component must be a plain name, and
//! the key is the granted prefix with the request joined below it. A bucket
//! root is written as a URL, `s3://bucket/prefix`, the same spelling a cloud
//! volume's mount point uses, so a grant's subtree says which bucket it lends.
//!
//! Writes reach a bucket only when its service can refuse a stale one. A
//! service without conditional writes would overwrite whatever changed since
//! the agent read it, so its grants are read-only for agents.

use std::{
	path::{Component, Path, PathBuf},
	sync::Arc,
};

use super::{scope::Resolved, GrantError, GrantScope};
use crate::{
	context::CoreContext,
	volume::{backend::CloudServiceType, RawDirEntry, VolumeBackend, VolumeError},
};

/// A bucket URL taken apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BucketUrl {
	pub service: CloudServiceType,
	pub bucket: String,
	/// The key prefix below the bucket, `/`-separated, without a leading or
	/// trailing `/`. Empty for the whole bucket.
	pub key: String,
}

impl BucketUrl {
	/// Parse `scheme://bucket/a/b`. A trailing `/` is dropped. `None` for
	/// anything that is not a bucket URL, such as a filesystem path.
	pub fn parse(path: &Path) -> Option<Self> {
		let text = path.to_str()?;
		let (scheme, rest) = text.split_once("://")?;
		let service = CloudServiceType::from_scheme(scheme)?;
		let rest = rest.trim_end_matches('/');
		let (bucket, key) = rest.split_once('/').unwrap_or((rest, ""));
		if bucket.is_empty() {
			return None;
		}
		if !key.is_empty()
			&& key
				.split('/')
				.any(|part| part.is_empty() || part == "." || part == "..")
		{
			return None;
		}
		Some(Self {
			service,
			bucket: bucket.to_string(),
			key: key.to_string(),
		})
	}

	/// The URL of the bucket itself, which is its cloud volume's mount point.
	pub fn volume(&self) -> String {
		format!("{}://{}", self.service.scheme(), self.bucket)
	}

	/// The canonical spelling: no trailing `/`, no empty components.
	pub fn path(&self) -> PathBuf {
		if self.key.is_empty() {
			PathBuf::from(self.volume())
		} else {
			PathBuf::from(format!("{}/{}", self.volume(), self.key))
		}
	}

	/// The key `relative` names below this one.
	pub fn join(&self, relative: &str) -> Self {
		let key = match (self.key.is_empty(), relative.is_empty()) {
			(_, true) => self.key.clone(),
			(true, false) => relative.to_string(),
			(false, false) => format!("{}/{relative}", self.key),
		};
		Self {
			key,
			..self.clone()
		}
	}

	/// Whether `self` is `other` or a prefix of it by whole components.
	pub fn contains(&self, other: &Self) -> bool {
		self.service == other.service
			&& self.bucket == other.bucket
			&& (self.key.is_empty()
				|| other.key == self.key
				|| other
					.key
					.strip_prefix(&self.key)
					.is_some_and(|rest| rest.starts_with('/')))
	}
}

/// Resolve `requested` below the granted prefix `root`.
///
/// `requested` is relative to the prefix, or the full bucket URL of a key
/// inside it. Anything but plain names is refused.
pub fn resolve(root: &BucketUrl, requested: &Path) -> Result<Resolved, GrantError> {
	let escape = || GrantError::Escape(requested.display().to_string());
	let relative = match BucketUrl::parse(requested) {
		Some(url) => {
			if !root.contains(&url) {
				return Err(escape());
			}
			url.key
				.strip_prefix(&root.key)
				.unwrap_or(&url.key)
				.trim_start_matches('/')
				.to_string()
		}
		None => {
			let mut parts = Vec::new();
			for component in requested.components() {
				match component {
					Component::Normal(name) => parts.push(
						name.to_str()
							.ok_or_else(|| GrantError::InvalidPath("names must be UTF-8".into()))?
							.to_string(),
					),
					Component::CurDir => {}
					_ => return Err(escape()),
				}
			}
			parts.join("/")
		}
	};
	Ok(Resolved {
		root: root.path(),
		path: root.join(&relative).path(),
		relative: PathBuf::from(relative),
	})
}

/// The bucket key an authorized scope names, or `None` for a filesystem
/// scope.
pub fn of_scope(scope: &GrantScope) -> Option<BucketUrl> {
	BucketUrl::parse(&scope.path)
}

/// The backend serving `url`'s bucket on this device.
pub async fn backend(
	context: &CoreContext,
	url: &BucketUrl,
) -> Result<Arc<dyn VolumeBackend>, GrantError> {
	context
		.volume_manager
		.find_cloud_volume(url.service, &url.bucket)
		.await
		.and_then(|volume| volume.backend)
		.ok_or_else(|| {
			GrantError::RootUnavailable(format!(
				"{} is not connected on this device; add it with volumes.add_cloud",
				url.volume()
			))
		})
}

/// The children of the prefix `url` names, sorted by name. A prefix with
/// nothing under it lists as empty.
pub async fn list(
	backend: &dyn VolumeBackend,
	url: &BucketUrl,
) -> Result<Vec<RawDirEntry>, VolumeError> {
	let prefix = if url.key.is_empty() {
		"/".to_string()
	} else {
		format!("{}/", url.key)
	};
	let mut entries = match backend.read_dir(Path::new(&prefix)).await {
		Ok(entries) => entries,
		Err(VolumeError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
		Err(error) => return Err(error),
	};
	entries.retain(|entry| !entry.name.is_empty());
	entries.sort_by(|a, b| a.name.cmp(&b.name));
	entries.dedup_by(|a, b| a.name == b.name);
	Ok(entries)
}

/// Refuse a writable request on a bucket whose service cannot refuse a
/// stale write.
pub fn writable(url: &BucketUrl, backend: &dyn VolumeBackend) -> Result<(), GrantError> {
	if backend.conditional_writes() {
		return Ok(());
	}
	Err(GrantError::NoConditionalWrites(url.volume()))
}

#[cfg(test)]
mod tests {
	use super::*;

	fn url(text: &str) -> BucketUrl {
		BucketUrl::parse(Path::new(text)).expect("a bucket URL")
	}

	#[test]
	fn urls_parse_and_print_canonically() {
		let prefixed = url("s3://media/projects/2026/");
		assert_eq!(prefixed.bucket, "media");
		assert_eq!(prefixed.key, "projects/2026");
		assert_eq!(prefixed.path(), PathBuf::from("s3://media/projects/2026"));
		assert_eq!(url("s3://media").key, "");
		assert!(BucketUrl::parse(Path::new("/home/me")).is_none());
		assert!(BucketUrl::parse(Path::new("s3://")).is_none());
		assert!(BucketUrl::parse(Path::new("s3://media/a/../b")).is_none());
		assert!(BucketUrl::parse(Path::new("s3://media/a//b")).is_none());
		assert!(BucketUrl::parse(Path::new("ftp://media/a")).is_none());
	}

	#[test]
	fn containment_is_by_whole_components() {
		let root = url("s3://media/projects");
		assert!(root.contains(&url("s3://media/projects")));
		assert!(root.contains(&url("s3://media/projects/a.txt")));
		assert!(!root.contains(&url("s3://media/projects-old/a.txt")));
		assert!(!root.contains(&url("s3://other/projects/a.txt")));
		assert!(url("s3://media").contains(&url("s3://media/anything")));
	}

	#[test]
	fn requests_resolve_below_the_prefix_and_never_above_it() {
		let root = url("s3://media/projects");
		let resolved = resolve(&root, Path::new("notes/today.md")).expect("inside");
		assert_eq!(
			resolved.path,
			PathBuf::from("s3://media/projects/notes/today.md")
		);
		assert_eq!(resolved.relative, PathBuf::from("notes/today.md"));
		let whole = resolve(&root, Path::new("")).expect("the root");
		assert_eq!(whole.path, root.path());
		assert!(resolve(&root, Path::new("../secrets")).is_err());
		assert!(resolve(&root, Path::new("/etc/passwd")).is_err());
		assert!(resolve(&root, Path::new("s3://media/other/a")).is_err());
		assert_eq!(
			resolve(&root, Path::new("s3://media/projects/a/b"))
				.expect("a full URL inside")
				.relative,
			PathBuf::from("a/b")
		);
	}
}
