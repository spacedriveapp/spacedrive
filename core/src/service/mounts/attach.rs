//! Mounting the SMB share on this machine, so apps outside Spacedrive can open
//! files from other devices by path.
//!
//! Quick Look, QuickTime and every other app the system hands a file to read it
//! by path. For a file whose bytes live on another device, the daemon mounts its
//! own loopback SMB share at `<data dir>/mount` and hands out the file's path
//! inside it, so the app's reads go through the byte plane and its block cache
//! like every other read of that file. The share is mounted the first time such
//! a path is asked for, read-only, soft and hidden from Finder, and unmounted
//! when the daemon shuts down.
//!
//! A daemon that exits without shutting down, which a signal or a killed app
//! causes, leaves the mount pointing at a server that no longer exists. Startup
//! unmounts whatever is at the mount point before anything lists mounts.

use std::path::{Path, PathBuf};

use crate::context::CoreContext;

/// The directory beneath the data dir the share is mounted at.
const MOUNT_DIR: &str = "mount";

/// One mount or unmount at a time, so two callers asking for a path at once
/// get one mount between them.
static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Debug, thiserror::Error)]
pub enum AttachError {
	#[error("the mounts share is not running")]
	NotServing,
	#[error("opening files from another device is not available on this platform yet")]
	Unsupported,
	#[error("mounting the share failed: {0}")]
	Mount(String),
}

/// Where the share is mounted for the daemon whose data dir is `data_dir`.
pub fn mount_point(data_dir: &Path) -> PathBuf {
	data_dir.join(MOUNT_DIR)
}

/// The directory the share is mounted at, mounting it first when it is not.
pub async fn attach(context: &CoreContext) -> Result<PathBuf, AttachError> {
	let _lock = LOCK.lock().await;
	let point = mount_point(&context.data_dir);
	let login = super::smb::login().ok_or(AttachError::NotServing)?;
	let mounted = point.clone();
	tokio::task::spawn_blocking(move || platform::attach(&mounted, &login))
		.await
		.map_err(|error| AttachError::Mount(error.to_string()))??;
	Ok(point)
}

/// Unmount whatever is mounted at the mount point: this run's share at
/// shutdown, or one an earlier run left behind at startup.
pub async fn detach(data_dir: &Path) {
	let _lock = LOCK.lock().await;
	let point = mount_point(data_dir);
	match tokio::task::spawn_blocking(move || platform::detach(&point)).await {
		Ok(Ok(true)) => tracing::info!("Unmounted the mounts share"),
		Ok(Ok(false)) => {}
		Ok(Err(error)) => tracing::warn!("Unmounting the mounts share failed: {error}"),
		Err(error) => tracing::warn!("Unmounting the mounts share failed: {error}"),
	}
}

#[cfg(target_os = "macos")]
mod platform {
	use std::ffi::{c_void, CStr, CString};
	use std::io;
	use std::os::unix::ffi::OsStrExt;
	use std::path::{Path, PathBuf};

	use objc2::rc::{autoreleasepool, Retained};
	use objc2_foundation::{
		ns_string, NSArray, NSMutableDictionary, NSNumber, NSObject, NSString, NSURL,
	};

	use super::AttachError;
	use crate::service::mounts::smb::Login;

	#[link(name = "NetFS", kind = "framework")]
	unsafe extern "C" {
		/// `NetFSMountURLSync` from `NetFS/NetFS.h`. Every object argument is
		/// a Core Foundation type, passed here as its toll-free bridged
		/// Foundation counterpart.
		fn NetFSMountURLSync(
			url: *const c_void,
			mountpath: *const c_void,
			user: *const c_void,
			passwd: *const c_void,
			open_options: *const c_void,
			mount_options: *const c_void,
			mountpoints: *mut *const c_void,
		) -> i32;
	}

	/// A filesystem in the mount table.
	struct Mounted {
		/// Where it is mounted, spelled the way the kernel spells it.
		on: PathBuf,
		/// What is mounted, as `mount(8)` prints it.
		from: String,
	}

	pub fn attach(point: &Path, login: &Login) -> Result<(), AttachError> {
		match mounted_at(point) {
			Some(mounted) if mounted.from == login.source => return Ok(()),
			Some(mounted) => {
				unmount(&mounted.on).map_err(|error| AttachError::Mount(error.to_string()))?
			}
			None => {}
		}
		std::fs::create_dir_all(point).map_err(|error| AttachError::Mount(error.to_string()))?;
		mount(point, login)
	}

	/// Whether something was mounted at `point` and is not any more.
	pub fn detach(point: &Path) -> io::Result<bool> {
		let Some(mounted) = mounted_at(point) else {
			return Ok(false);
		};
		unmount(&mounted.on)?;
		Ok(true)
	}

	fn mount(point: &Path, login: &Login) -> Result<(), AttachError> {
		let point = point
			.to_str()
			.ok_or_else(|| AttachError::Mount("the mount point is not UTF-8".to_string()))?;
		let status = autoreleasepool(|_| {
			let url = NSURL::URLWithString(&NSString::from_str(&login.url))
				.ok_or_else(|| AttachError::Mount(format!("invalid share URL {}", login.url)))?;
			let mount_path = NSURL::fileURLWithPath_isDirectory(&NSString::from_str(point), true);
			let user = NSString::from_str(login.user);
			let password = NSString::from_str(login.password);

			// Nothing may prompt: the daemon has no one to ask.
			let open_options = NSMutableDictionary::<NSString, NSObject>::new();
			open_options.insert(ns_string!("UIOption"), ns_string!("NoUI"));

			// Mounted at the directory given rather than one NetFS creates
			// under /Volumes. Soft, so a read fails instead of hanging when
			// the server is gone. Read-only like the share, and hidden from
			// Finder's sidebar and desktop.
			let mount_options = NSMutableDictionary::<NSString, NSObject>::new();
			let yes = NSNumber::new_bool(true);
			mount_options.insert(ns_string!("MountAtMountDir"), &yes);
			mount_options.insert(ns_string!("SoftMount"), &yes);
			mount_options.insert(
				ns_string!("MountFlags"),
				&NSNumber::new_i32(libc::MNT_DONTBROWSE | libc::MNT_RDONLY),
			);

			let mut mountpoints: *const c_void = std::ptr::null();
			// SAFETY: every pointer is a live object of the bridged type the
			// function takes, borrowed for the length of the call.
			let status = unsafe {
				NetFSMountURLSync(
					Retained::as_ptr(&url).cast(),
					Retained::as_ptr(&mount_path).cast(),
					Retained::as_ptr(&user).cast(),
					Retained::as_ptr(&password).cast(),
					Retained::as_ptr(&open_options).cast(),
					Retained::as_ptr(&mount_options).cast(),
					&mut mountpoints,
				)
			};
			// SAFETY: the array follows the create rule, so the caller owns it
			// and releasing it here is the one release it needs.
			drop(unsafe { Retained::<NSArray>::from_raw(mountpoints.cast_mut().cast()) });
			Ok(status)
		})?;
		match status {
			0 => Ok(()),
			code if code > 0 => Err(AttachError::Mount(
				io::Error::from_raw_os_error(code).to_string(),
			)),
			code => Err(AttachError::Mount(format!("NetFS error {code}"))),
		}
	}

	fn unmount(on: &Path) -> io::Result<()> {
		let path = CString::new(on.as_os_str().as_bytes())?;
		// Forced: this is the daemon's own share, and an app holding a file
		// open in it has lost the server either way.
		// SAFETY: `path` is a NUL-terminated string that outlives the call.
		if unsafe { libc::unmount(path.as_ptr(), libc::MNT_FORCE) } == 0 {
			Ok(())
		} else {
			Err(io::Error::last_os_error())
		}
	}

	/// What is mounted at `point`, read from the mount table without asking
	/// any filesystem, since a mount left by an earlier run has no server.
	/// The kernel may spell the point under either firmlink, so every
	/// spelling is built, from the parent directory rather than the point
	/// itself, which resolving could ask a dead server about.
	fn mounted_at(point: &Path) -> Option<Mounted> {
		let spellings: Vec<PathBuf> = match (point.parent(), point.file_name()) {
			(Some(parent), Some(name)) => crate::config::path_spellings(parent)
				.into_iter()
				.map(|dir| dir.join(name))
				.collect(),
			_ => vec![point.to_path_buf()],
		};
		mount_table()
			.into_iter()
			.find(|mounted| spellings.contains(&mounted.on))
	}

	fn mount_table() -> Vec<Mounted> {
		// SAFETY: a null buffer asks only for the number of mounts.
		let count = unsafe { libc::getfsstat(std::ptr::null_mut(), 0, libc::MNT_NOWAIT) };
		if count <= 0 {
			return Vec::new();
		}
		// Room for mounts that appear between the two calls.
		let capacity = count as usize + 8;
		let mut table: Vec<libc::statfs> = Vec::with_capacity(capacity);
		let size = (capacity * std::mem::size_of::<libc::statfs>()) as libc::c_int;
		// SAFETY: the buffer holds `capacity` entries and `size` says so.
		let filled = unsafe { libc::getfsstat(table.as_mut_ptr(), size, libc::MNT_NOWAIT) };
		if filled <= 0 {
			return Vec::new();
		}
		// SAFETY: getfsstat initialised the first `filled` entries.
		unsafe { table.set_len(filled as usize) };
		table
			.iter()
			.map(|entry| Mounted {
				on: PathBuf::from(c_string(&entry.f_mntonname)),
				from: c_string(&entry.f_mntfromname),
			})
			.collect()
	}

	fn c_string(chars: &[libc::c_char]) -> String {
		// SAFETY: the kernel NUL-terminates every name field in `statfs`.
		unsafe { CStr::from_ptr(chars.as_ptr()) }
			.to_string_lossy()
			.into_owned()
	}
}

#[cfg(not(target_os = "macos"))]
mod platform {
	use std::io;
	use std::path::Path;

	use super::AttachError;
	use crate::service::mounts::smb::Login;

	pub fn attach(_point: &Path, _login: &Login) -> Result<(), AttachError> {
		Err(AttachError::Unsupported)
	}

	pub fn detach(_point: &Path) -> io::Result<bool> {
		Ok(false)
	}
}
