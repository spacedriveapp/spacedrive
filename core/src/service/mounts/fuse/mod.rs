//! # Read-only FUSE mount (Linux)
//!
//! `service::mounts::fuse` presents every source as a plain directory tree at
//! a path the user picks, so agents and command-line tools can use ordinary
//! file calls on files that may live on another device. It is a third
//! frontend over the same byte plane as the WebDAV and SMB shares: listings
//! and attributes come from each source's arena, and bytes come from the
//! source's provider, so a peer's file streams through the byterange protocol
//! and the block cache and is fetched only where it is read.
//!
//! Mounts are made on request and belong to the daemon. A mount can be bound
//! to an agent grant, in which case it shows only the granted directory and
//! checks the grant's token on every listing and open, so a revoked or expired
//! grant is refused at once and its mount is taken down within seconds. Every
//! mount is unmounted when the daemon shuts down. The mount needs the `fuse`
//! cargo feature and `mounts.fuse_enabled` in the daemon's config; without
//! either, mounting is refused with a reason. `docs/core/fuse-mount.mdx`
//! covers use and behavior.
//!
//! ## Example
//! ```rust,no_run
//! # async fn demo(context: std::sync::Arc<sd_core::context::CoreContext>) {
//! use sd_core::service::mounts::fuse;
//!
//! let info = fuse::mount(context, "/home/me/sd".into(), None).await;
//! # }
//! ```

#[cfg(all(target_os = "linux", feature = "fuse"))]
pub mod fs;
pub use super::tree;

use crate::config::AppConfig;
use crate::context::CoreContext;
use crate::ops::grants::{create::owning_source, GrantError};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};
use thiserror::Error;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tracing::{info, warn};
use uuid::Uuid;

/// How often a grant-bound mount checks that its grant is still active.
#[cfg(all(target_os = "linux", feature = "fuse"))]
const GRANT_CHECK_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);

/// A mount the daemon holds.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FuseMountInfo {
	pub path: PathBuf,
	/// The grant the mount is bound to, when it shows only one directory.
	pub grant_id: Option<Uuid>,
	/// The granted directory the mount shows.
	pub granted_path: Option<PathBuf>,
	pub expires_at: Option<DateTime<Utc>>,
	pub mounted_at: DateTime<Utc>,
}

#[derive(Debug, Error)]
pub enum FuseError {
	#[error("this daemon has no FUSE support; it needs Linux and a build with the fuse feature")]
	Unsupported,
	#[error("FUSE mounts are off; turn them on with: sd-cli config set mounts.fuse_enabled true")]
	Disabled,
	#[error("cannot mount at {0}")]
	Mountpoint(String),
	#[error("{0} is already mounted")]
	AlreadyMounted(PathBuf),
	#[error("nothing is mounted at {0}")]
	NotMounted(PathBuf),
	#[error("the grant refused the mount: {0}")]
	Grant(#[from] GrantError),
	#[error("the granted directory is in no source on this device")]
	NoSource,
	#[error("mount failed: {0}")]
	Mount(String),
}

#[cfg(all(target_os = "linux", feature = "fuse"))]
type Session = fs::Mounted;
#[cfg(not(all(target_os = "linux", feature = "fuse")))]
type Session = std::convert::Infallible;

#[cfg_attr(not(all(target_os = "linux", feature = "fuse")), allow(dead_code))]
struct Mounted {
	info: FuseMountInfo,
	session: Session,
	/// Takes the mount down when its grant ends.
	watch: Option<JoinHandle<()>>,
}

static MOUNTS: LazyLock<Mutex<HashMap<PathBuf, Mounted>>> =
	LazyLock::new(|| Mutex::new(HashMap::new()));

/// Whether this build can mount at all.
pub fn supported() -> bool {
	cfg!(all(target_os = "linux", feature = "fuse"))
}

/// Whether the daemon's config allows mounts. Read from disk so a change
/// through the config op applies without a restart.
pub async fn enabled(context: &CoreContext) -> bool {
	let data_dir = context.data_dir.clone();
	tokio::task::spawn_blocking(move || AppConfig::load_from(&data_dir))
		.await
		.ok()
		.and_then(Result::ok)
		.is_some_and(|config| config.mounts.fuse_enabled)
}

/// The mounts the daemon holds now.
pub async fn list() -> Vec<FuseMountInfo> {
	let mut mounts: Vec<FuseMountInfo> = MOUNTS
		.lock()
		.await
		.values()
		.map(|mounted| mounted.info.clone())
		.collect();
	mounts.sort_by(|a, b| a.path.cmp(&b.path));
	mounts
}

/// Mount every source at `path`, or with `grant_token` only the directory
/// that grant lends. `path` must be an existing empty directory.
pub async fn mount(
	context: Arc<CoreContext>,
	path: PathBuf,
	grant_token: Option<String>,
) -> Result<FuseMountInfo, FuseError> {
	if !supported() {
		return Err(FuseError::Unsupported);
	}
	if !enabled(&context).await {
		return Err(FuseError::Disabled);
	}
	mount_tree(context, path, grant_token).await
}

#[cfg(not(all(target_os = "linux", feature = "fuse")))]
async fn mount_tree(
	_context: Arc<CoreContext>,
	_path: PathBuf,
	_grant_token: Option<String>,
) -> Result<FuseMountInfo, FuseError> {
	Err(FuseError::Unsupported)
}

#[cfg(all(target_os = "linux", feature = "fuse"))]
async fn mount_tree(
	context: Arc<CoreContext>,
	path: PathBuf,
	grant_token: Option<String>,
) -> Result<FuseMountInfo, FuseError> {
	use crate::ops::grants::{authorize, Access, GrantCap};
	use tree::{ArenaFs, BoundGrant, LibraryTree, MountRoot};

	let path = prepare_mountpoint(path).await?;
	let mut mounts = MOUNTS.lock().await;
	if mounts.contains_key(&path) {
		return Err(FuseError::AlreadyMounted(path));
	}

	let (grant, scope) = match grant_token {
		Some(token) => {
			let access = Access::new(&token, "fuse.mount", Path::new(""), GrantCap::Read);
			let scope = authorize(&context, &access).await?.scope;
			// The grant keeps the directory's canonical spelling, which on
			// macOS can differ from the source's by a firmlink, and the tree
			// answers only to the source's spelling. A bucket grant has no
			// directory to compare, so its URL is matched by prefix.
			let sources = context.volume_index().sources();
			let (source_id, path) = match owning_source(&context, scope.library_id, &scope.root)
				.await
				.map_err(FuseError::Mount)?
			{
				Some(found) => found,
				None => crate::ops::paths::reach::innermost_source(&sources, &scope.root)
					.map(|source| (source.id, scope.root.clone()))
					.ok_or(FuseError::NoSource)?,
			};
			let grant = BoundGrant {
				token: token.clone(),
				root: MountRoot {
					name: String::new(),
					source_id,
					path,
					remote: false,
				},
			};
			(Some(grant), Some((scope, token)))
		}
		None => (None, None),
	};

	let tree = Arc::new(ArenaFs::new(Arc::new(LibraryTree::new(
		context.clone(),
		grant,
	))));
	let session = fs::mount(tree, &path)
		.await
		.map_err(|err| FuseError::Mount(err.to_string()))?;

	let info = FuseMountInfo {
		path: path.clone(),
		grant_id: scope.as_ref().map(|(scope, _)| scope.grant_id),
		granted_path: scope.as_ref().map(|(scope, _)| scope.root.clone()),
		expires_at: scope.as_ref().map(|(scope, _)| scope.expires_at),
		mounted_at: Utc::now(),
	};
	let watch = scope.map(|(scope, token)| {
		tokio::spawn(watch_grant(
			context.clone(),
			path.clone(),
			scope.library_id,
			scope.grant_id,
			token,
		))
	});
	info!(path = %path.display(), grant = ?info.grant_id, "FUSE mount up");
	mounts.insert(
		path,
		Mounted {
			info: info.clone(),
			session,
			watch,
		},
	);
	Ok(info)
}

/// The mount point, absolute and canonical, once it is known to be an empty
/// directory. A mount left behind by a daemon that died answers every call
/// with ENOTCONN; it is detached first so the path can be reused.
#[cfg(all(target_os = "linux", feature = "fuse"))]
pub async fn prepare_mountpoint(path: PathBuf) -> Result<PathBuf, FuseError> {
	tokio::task::spawn_blocking(move || {
		if !path.is_absolute() {
			return Err(FuseError::Mountpoint(format!(
				"{} is not an absolute path",
				path.display()
			)));
		}
		if let Err(err) = std::fs::metadata(&path) {
			if err.raw_os_error() == Some(libc::ENOTCONN) {
				warn!(path = %path.display(), "detaching a stale FUSE mount");
				lazy_unmount(&path);
			}
		}
		let meta = std::fs::metadata(&path)
			.map_err(|err| FuseError::Mountpoint(format!("{}: {err}", path.display())))?;
		if !meta.is_dir() {
			return Err(FuseError::Mountpoint(format!(
				"{} is not a directory",
				path.display()
			)));
		}
		let mut entries = std::fs::read_dir(&path)
			.map_err(|err| FuseError::Mountpoint(format!("{}: {err}", path.display())))?;
		if entries.next().is_some() {
			return Err(FuseError::Mountpoint(format!(
				"{} is not empty",
				path.display()
			)));
		}
		std::fs::canonicalize(&path)
			.map_err(|err| FuseError::Mountpoint(format!("{}: {err}", path.display())))
	})
	.await
	.map_err(|err| FuseError::Mount(err.to_string()))?
}

/// Detach a mount even while something still has a file open in it.
#[cfg(all(target_os = "linux", feature = "fuse"))]
fn lazy_unmount(path: &Path) {
	let status = std::process::Command::new("fusermount3")
		.arg("-u")
		.arg("-z")
		.arg(path)
		.status();
	if !matches!(status, Ok(status) if status.success()) {
		warn!(path = %path.display(), ?status, "fusermount3 -u -z failed");
	}
}

/// Take the mount down once its grant is revoked, expires, or disappears.
/// Each open already checks the grant; this makes the mount itself go away.
#[cfg(all(target_os = "linux", feature = "fuse"))]
async fn watch_grant(
	context: Arc<CoreContext>,
	path: PathBuf,
	library_id: Uuid,
	grant_id: Uuid,
	token: String,
) {
	use crate::ops::grants::{verified, GrantError, GrantStatus};

	loop {
		tokio::time::sleep(GRANT_CHECK_INTERVAL).await;
		match verified(&context, library_id, &token).await {
			Ok(grant) if grant.status(Utc::now()) == GrantStatus::Active => continue,
			Ok(_) | Err(GrantError::UnknownToken) => break,
			Err(err) => warn!(grant = %grant_id, error = %err, "could not read a mounted grant"),
		}
	}
	info!(path = %path.display(), grant = %grant_id, "grant ended; unmounting");
	if let Err(err) = take_down(&path, false).await {
		warn!(path = %path.display(), error = %err, "could not unmount after the grant ended");
	}
}

/// Unmount what the daemon mounted at `path`.
pub async fn unmount(path: &Path) -> Result<FuseMountInfo, FuseError> {
	let path = tokio::fs::canonicalize(path)
		.await
		.unwrap_or_else(|_| path.to_path_buf());
	take_down(&path, true).await
}

async fn take_down(path: &Path, stop_watch: bool) -> Result<FuseMountInfo, FuseError> {
	let mounted = MOUNTS
		.lock()
		.await
		.remove(path)
		.ok_or_else(|| FuseError::NotMounted(path.to_path_buf()))?;
	if stop_watch {
		if let Some(watch) = &mounted.watch {
			watch.abort();
		}
	}
	end_session(path, mounted.session).await;
	info!(path = %path.display(), "FUSE mount down");
	Ok(mounted.info)
}

#[cfg(all(target_os = "linux", feature = "fuse"))]
async fn end_session(path: &Path, session: Session) {
	let path = path.to_path_buf();
	let _ = tokio::task::spawn_blocking(move || {
		if let Err(err) = session.umount_and_join() {
			// Something still holds the mount; detach it so it ends once
			// that last user goes away.
			warn!(path = %path.display(), error = %err, "unmount refused; detaching lazily");
			lazy_unmount(&path);
		}
	})
	.await;
}

#[cfg(not(all(target_os = "linux", feature = "fuse")))]
async fn end_session(_path: &Path, session: Session) {
	match session {}
}

/// Unmount everything, for daemon shutdown.
pub async fn unmount_all() {
	let paths: Vec<PathBuf> = MOUNTS.lock().await.keys().cloned().collect();
	for path in paths {
		if let Err(err) = take_down(&path, true).await {
			warn!(path = %path.display(), error = %err, "could not unmount at shutdown");
		}
	}
}
