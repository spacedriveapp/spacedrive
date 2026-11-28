//! # Read-only NFS mount (macOS and Linux)
//!
//! `service::mounts::nfs` serves the same tree as the FUSE mount over a
//! localhost NFSv3 server, which the OS's built-in NFS client mounts. macOS
//! has no FUSE without a kernel extension, and its NFS client needs neither
//! an extension nor an entitlement, so this is how a Mac gets a plain
//! directory of every source. Linux can mount it too, as root, which is how
//! the tests exercise it there.
//!
//! The server binds 127.0.0.1 on a port the OS picks. NFSv3 has no real
//! authentication, so any local user who finds the port can read what the
//! mount shows. Mounts are therefore off until `mounts.nfs_enabled` is set,
//! and a mount bound to an agent grant checks the grant on every listing and
//! open just as the FUSE mount does, and is taken down when the grant ends.
//! Every mount is unmounted when the daemon shuts down.
//! [`server`] holds the protocol side, which needs no `CoreContext`.
//! `docs/core/nfs-mount.mdx` covers use and behavior.
//!
//! ## Example
//! ```rust,no_run
//! # async fn demo(context: std::sync::Arc<sd_core::context::CoreContext>) {
//! use sd_core::service::mounts::nfs;
//!
//! let info = nfs::mount(context, "/Users/me/sd".into(), None).await;
//! # }
//! ```

#[cfg(feature = "nfs")]
mod front;
#[cfg(feature = "nfs")]
pub mod server;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tracing::{info, warn};
use uuid::Uuid;

use crate::config::AppConfig;
use crate::context::CoreContext;
use crate::ops::grants::GrantError;

/// An NFS mount the daemon holds.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct NfsMountInfo {
	pub path: PathBuf,
	/// The loopback port the mount's server answers on.
	pub port: u16,
	/// The grant the mount is bound to, when it shows only one directory.
	pub grant_id: Option<Uuid>,
	/// The granted directory the mount shows.
	pub granted_path: Option<PathBuf>,
	pub expires_at: Option<DateTime<Utc>>,
	pub mounted_at: DateTime<Utc>,
}

#[derive(Debug, Error)]
pub enum NfsError {
	#[error("this daemon has no NFS mount support; it needs macOS or Linux and a build with the nfs feature")]
	Unsupported,
	#[error("NFS mounts are off; turn them on with: sd-cli config set mounts.nfs_enabled true")]
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

#[cfg(all(unix, feature = "nfs"))]
impl From<super::mountpoint::Refusal> for NfsError {
	fn from(refusal: super::mountpoint::Refusal) -> Self {
		use super::mountpoint::Refusal;
		match refusal {
			Refusal::Mountpoint(reason) => Self::Mountpoint(reason),
			Refusal::Grant(err) => Self::Grant(err),
			Refusal::NoSource => Self::NoSource,
			Refusal::Lookup(reason) => Self::Mount(reason),
		}
	}
}

#[cfg(all(unix, feature = "nfs"))]
type Session = server::NfsServer;
#[cfg(not(all(unix, feature = "nfs")))]
type Session = std::convert::Infallible;

#[cfg_attr(not(all(unix, feature = "nfs")), allow(dead_code))]
struct Mounted {
	info: NfsMountInfo,
	session: Session,
	/// Takes the mount down when its grant ends.
	watch: Option<JoinHandle<()>>,
}

static MOUNTS: LazyLock<Mutex<HashMap<PathBuf, Mounted>>> =
	LazyLock::new(|| Mutex::new(HashMap::new()));

/// Whether this build can mount at all.
pub fn supported() -> bool {
	cfg!(all(unix, feature = "nfs"))
}

/// Whether the daemon's config allows mounts. Read from disk so a change
/// through the config op applies without a restart.
pub async fn enabled(context: &CoreContext) -> bool {
	let data_dir = context.data_dir.clone();
	tokio::task::spawn_blocking(move || AppConfig::load_from(&data_dir))
		.await
		.ok()
		.and_then(Result::ok)
		.is_some_and(|config| config.mounts.nfs_enabled)
}

/// The mounts the daemon holds now.
pub async fn list() -> Vec<NfsMountInfo> {
	let mut mounts: Vec<NfsMountInfo> = MOUNTS
		.lock()
		.await
		.values()
		.map(|mounted| mounted.info.clone())
		.collect();
	mounts.sort_by(|a, b| a.path.cmp(&b.path));
	mounts
}

/// `path`, absolute and canonical, once it is known to be an empty directory
/// to mount at.
#[cfg(all(unix, feature = "nfs"))]
pub async fn prepare_mountpoint(path: PathBuf) -> Result<PathBuf, NfsError> {
	tokio::task::spawn_blocking(move || Ok(super::mountpoint::empty_dir(&path)?))
		.await
		.map_err(|err| NfsError::Mount(err.to_string()))?
}

/// Mount every source at `path`, or with `grant_token` only the directory
/// that grant lends. `path` must be an existing empty directory.
pub async fn mount(
	context: Arc<CoreContext>,
	path: PathBuf,
	grant_token: Option<String>,
) -> Result<NfsMountInfo, NfsError> {
	if !supported() {
		return Err(NfsError::Unsupported);
	}
	if !enabled(&context).await {
		return Err(NfsError::Disabled);
	}
	mount_tree(context, path, grant_token).await
}

#[cfg(not(all(unix, feature = "nfs")))]
async fn mount_tree(
	_context: Arc<CoreContext>,
	_path: PathBuf,
	_grant_token: Option<String>,
) -> Result<NfsMountInfo, NfsError> {
	Err(NfsError::Unsupported)
}

#[cfg(all(unix, feature = "nfs"))]
async fn mount_tree(
	context: Arc<CoreContext>,
	path: PathBuf,
	grant_token: Option<String>,
) -> Result<NfsMountInfo, NfsError> {
	use super::mountpoint;
	use super::tree::{ArenaFs, LibraryTree};

	let path = tokio::task::spawn_blocking(move || mountpoint::empty_dir(&path))
		.await
		.map_err(|err| NfsError::Mount(err.to_string()))??;
	let mut mounts = MOUNTS.lock().await;
	if mounts.contains_key(&path) {
		return Err(NfsError::AlreadyMounted(path));
	}

	let (grant, scope) = match grant_token {
		Some(token) => {
			let (grant, scope) =
				mountpoint::bind_grant(&context, token.clone(), "nfs.mount").await?;
			(Some(grant), Some((scope, token)))
		}
		None => (None, None),
	};

	let tree = Arc::new(ArenaFs::new(Arc::new(LibraryTree::new(
		context.clone(),
		grant,
		"nfs",
	))));
	let session = server::serve(tree)
		.await
		.map_err(|err| NfsError::Mount(err.to_string()))?;
	server::mount(&session, &path)
		.await
		.map_err(|err| NfsError::Mount(err.to_string()))?;

	let info = NfsMountInfo {
		path: path.clone(),
		port: session.port(),
		grant_id: scope.as_ref().map(|(scope, _)| scope.grant_id),
		granted_path: scope.as_ref().map(|(scope, _)| scope.root.clone()),
		expires_at: scope.as_ref().map(|(scope, _)| scope.expires_at),
		mounted_at: Utc::now(),
	};
	let watch = scope.map(|(scope, token)| {
		let context = context.clone();
		let path = path.clone();
		tokio::spawn(async move {
			mountpoint::grant_ended(context, scope.library_id, scope.grant_id, token).await;
			info!(path = %path.display(), grant = %scope.grant_id, "grant ended; unmounting");
			if let Err(err) = take_down(&path, false).await {
				warn!(path = %path.display(), error = %err, "could not unmount after the grant ended");
			}
		})
	});
	info!(path = %path.display(), grant = ?info.grant_id, port = info.port, "NFS mount up");
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

/// Unmount what the daemon mounted at `path`.
pub async fn unmount(path: &Path) -> Result<NfsMountInfo, NfsError> {
	let path = tokio::fs::canonicalize(path)
		.await
		.unwrap_or_else(|_| path.to_path_buf());
	take_down(&path, true).await
}

/// Detach the kernel mount, then stop its server. If the OS will not let
/// go, the server stops answering but the mount stays listed, so a later
/// unmount can try again.
async fn take_down(path: &Path, stop_watch: bool) -> Result<NfsMountInfo, NfsError> {
	let mut mounts = MOUNTS.lock().await;
	let mounted = mounts
		.get(path)
		.ok_or_else(|| NfsError::NotMounted(path.to_path_buf()))?;
	if let Err(err) = detach(path, &mounted.session).await {
		warn!(path = %path.display(), error = %err, "could not unmount; the server stops answering");
		return Err(err);
	}
	let mounted = mounts
		.remove(path)
		.ok_or_else(|| NfsError::NotMounted(path.to_path_buf()))?;
	drop(mounts);
	if stop_watch {
		if let Some(watch) = &mounted.watch {
			watch.abort();
		}
	}
	stop(mounted.session);
	info!(path = %path.display(), "NFS mount down");
	Ok(mounted.info)
}

#[cfg(all(unix, feature = "nfs"))]
async fn detach(path: &Path, session: &Session) -> Result<(), NfsError> {
	server::unmount(path).await.map_err(|err| {
		session.close();
		NfsError::Mount(err.to_string())
	})
}

#[cfg(all(unix, feature = "nfs"))]
fn stop(session: Session) {
	session.shutdown();
}

#[cfg(not(all(unix, feature = "nfs")))]
async fn detach(_path: &Path, session: &Session) -> Result<(), NfsError> {
	match *session {}
}

#[cfg(not(all(unix, feature = "nfs")))]
fn stop(session: Session) {
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
