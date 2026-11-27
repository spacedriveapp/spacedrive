//! # Path containment
//!
//! `core::common::contain` opens a file a remote party named relative to a
//! root the user chose, and guarantees the opened file is beneath that root.
//! Paired devices and agent grants send paths, and a path check followed by a
//! normal open is not enough: the kernel follows symlinks during the open, so
//! a link inside the root that points elsewhere reads outside it, and a link
//! swapped in between the check and the open defeats any check on the string.
//!
//! The answer is to decide containment during resolution and return the
//! handle that resolution produced. Linux resolves with `openat2` and
//! `RESOLVE_BENEATH`. Elsewhere, and on kernels without `openat2`, a walk
//! opens one component at a time from the root's directory handle with
//! `O_NOFOLLOW` and expands symlinks itself, refusing any that climb above the
//! root. Windows opens the path and compares the handle's final path with the
//! root's. Callers read only through the returned handle, so nothing can
//! change what was checked. `docs/core/design/agent-access-threat-model.md`
//! has the threat model.
//!
//! A symlink whose target stays beneath the root opens. One that leaves it
//! fails with `PermissionDenied`. Hard links and volumes mounted inside the
//! root are accepted risks, since both are under the folder the user chose.
//!
//! ## Example
//! ```rust,no_run
//! use std::io::Read;
//! use std::path::Path;
//! use sd_core::common::contain::open_beneath;
//!
//! let mut file = open_beneath(Path::new("/srv/photos"), Path::new("2024/a.jpg"))?;
//! let mut bytes = Vec::new();
//! file.read_to_end(&mut bytes)?;
//! # Ok::<(), std::io::Error>(())
//! ```

use std::fs::File;
use std::io;
use std::path::Path;

const MAX_COMPONENT_BYTES: usize = 255;

/// Opens `relative` read-only beneath `root`, refusing any resolution that
/// leaves it.
///
/// `relative` is validated before the filesystem is touched: absolute paths,
/// empty, `.` and `..` components, NUL and components over 255 bytes are
/// `InvalidInput`, as are, on Windows, `:`, a trailing dot or space and
/// reserved device names. Windows accepts `\\` as a separator like `/`.
/// `root` itself is trusted and may be a symlink or spelled differently from
/// its real path. Directories open too, so the handle can answer `metadata`
/// for either. On Unix the handle is non-blocking, so a FIFO opens at once
/// instead of waiting for a writer.
pub fn open_beneath(root: &Path, relative: &Path) -> io::Result<File> {
	let components = validate(relative)?;
	imp::open_beneath(root, &components)
}

/// Opens a trusted root itself, file or directory, the way
/// [`open_beneath`] opens what lies under it.
pub fn open_root(root: &Path) -> io::Result<File> {
	imp::open_root(root)
}

fn invalid(reason: &str) -> io::Error {
	io::Error::new(io::ErrorKind::InvalidInput, reason.to_string())
}

fn escaped() -> io::Error {
	io::Error::new(
		io::ErrorKind::PermissionDenied,
		"path resolves outside its root",
	)
}

#[cfg(unix)]
fn validate(relative: &Path) -> io::Result<Vec<std::ffi::OsString>> {
	use std::ffi::OsStr;
	use std::os::unix::ffi::OsStrExt;

	// Path::components hides empty and interior "." components, so split the
	// raw bytes to see exactly what was sent.
	let bytes = relative.as_os_str().as_bytes();
	if bytes.is_empty() {
		return Err(invalid("empty path"));
	}
	if bytes[0] == b'/' {
		return Err(invalid("absolute path"));
	}
	bytes
		.split(|b| *b == b'/')
		.map(|part| {
			check_component(part)?;
			Ok(OsStr::from_bytes(part).to_os_string())
		})
		.collect()
}

#[cfg(windows)]
fn validate(relative: &Path) -> io::Result<Vec<std::ffi::OsString>> {
	let text = relative
		.to_str()
		.ok_or_else(|| invalid("path is not valid Unicode"))?;
	if text.is_empty() {
		return Err(invalid("empty path"));
	}
	if text.starts_with(['/', '\\']) || text.contains(':') {
		return Err(invalid("absolute path or reserved character"));
	}
	text.split(['/', '\\'])
		.map(|part| {
			check_component(part.as_bytes())?;
			if part.ends_with('.') || part.ends_with(' ') {
				return Err(invalid("component ends in a dot or space"));
			}
			let stem = part.split('.').next().unwrap_or(part).trim_end();
			let upper = stem.to_ascii_uppercase();
			let reserved = matches!(
				upper.as_str(),
				"CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
			) || matches!(
				upper.as_bytes(),
				[b'C', b'O', b'M', digit] | [b'L', b'P', b'T', digit] if digit.is_ascii_digit()
			);
			if reserved {
				return Err(invalid("reserved device name"));
			}
			Ok(part.into())
		})
		.collect()
}

fn check_component(part: &[u8]) -> io::Result<()> {
	match part {
		b"" => Err(invalid("empty path component")),
		b"." | b".." => Err(invalid("relative path component")),
		_ if part.contains(&0) => Err(invalid("NUL in path")),
		_ if part.len() > MAX_COMPONENT_BYTES => Err(invalid("path component too long")),
		_ => Ok(()),
	}
}

#[cfg(unix)]
mod imp {
	use std::ffi::{CString, OsStr, OsString};
	use std::fs::File;
	use std::io;
	use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
	use std::os::unix::ffi::OsStrExt;
	use std::path::{Component, Path, PathBuf};

	use super::escaped;

	/// Symlink expansions allowed in one walk, matching Linux's own limit.
	const MAX_LINKS: usize = 40;

	pub fn open_root(root: &Path) -> io::Result<File> {
		File::open(root)
	}

	pub fn open_beneath(root: &Path, components: &[OsString]) -> io::Result<File> {
		// The root is trusted, so resolve its spelling once; absolute links
		// are judged against this real path.
		let real_root = std::fs::canonicalize(root)?;
		let root_fd = open_at(
			None,
			real_root.as_os_str(),
			libc::O_RDONLY | libc::O_DIRECTORY,
		)?;

		#[cfg(target_os = "linux")]
		match openat2_beneath(&root_fd, components) {
			Ok(file) => return Ok(file),
			// ENOSYS is a kernel without openat2. EXDEV is a link leaving
			// the root or an absolute link, which the walk judges against
			// the real root so an absolute link back inside still opens.
			Err(err) if matches!(err.raw_os_error(), Some(libc::ENOSYS) | Some(libc::EXDEV)) => {}
			Err(err) => return Err(err),
		}

		let file = walk(&root_fd, &real_root, components)?;
		#[cfg(target_os = "macos")]
		check_final_path(&root_fd, &file)?;
		Ok(file)
	}

	fn cstring(name: &OsStr) -> io::Result<CString> {
		CString::new(name.as_bytes()).map_err(|_| super::invalid("NUL in path"))
	}

	fn open_at(dir: Option<&OwnedFd>, name: &OsStr, flags: libc::c_int) -> io::Result<OwnedFd> {
		let name = cstring(name)?;
		let dir = dir.map_or(libc::AT_FDCWD, |fd| fd.as_raw_fd());
		let fd = unsafe { libc::openat(dir, name.as_ptr(), flags | libc::O_CLOEXEC) };
		if fd < 0 {
			return Err(io::Error::last_os_error());
		}
		Ok(unsafe { OwnedFd::from_raw_fd(fd) })
	}

	#[cfg(target_os = "linux")]
	fn openat2_beneath(root: &OwnedFd, components: &[OsString]) -> io::Result<File> {
		let joined: PathBuf = components.iter().collect();
		let path = cstring(joined.as_os_str())?;
		let mut how: libc::open_how = unsafe { std::mem::zeroed() };
		how.flags = (libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOCTTY | libc::O_NONBLOCK) as u64;
		how.resolve = libc::RESOLVE_BENEATH | libc::RESOLVE_NO_MAGICLINKS;
		// The kernel answers EAGAIN when a rename races the resolution.
		for _ in 0..8 {
			let fd = unsafe {
				libc::syscall(
					libc::SYS_openat2,
					root.as_raw_fd(),
					path.as_ptr(),
					&how as *const libc::open_how,
					std::mem::size_of::<libc::open_how>(),
				)
			};
			if fd >= 0 {
				return Ok(unsafe { File::from_raw_fd(fd as libc::c_int) });
			}
			let err = io::Error::last_os_error();
			if err.raw_os_error() != Some(libc::EAGAIN) {
				return Err(err);
			}
		}
		Err(io::Error::from_raw_os_error(libc::EAGAIN))
	}

	/// Resolves one component at a time from the root, never letting the
	/// kernel follow a link. Each directory handle stays open on a stack, so
	/// `..` from a link target pops back to its parent and an attempt to pop
	/// the root itself is an escape.
	pub(super) fn walk(
		root: &OwnedFd,
		real_root: &Path,
		components: &[OsString],
	) -> io::Result<File> {
		let mut stack: Vec<OwnedFd> = vec![root.try_clone()?];
		let mut pending: Vec<OsString> = components.iter().rev().cloned().collect();
		let mut links = 0;

		while let Some(name) = pending.pop() {
			if name == ".." {
				if stack.len() == 1 {
					return Err(escaped());
				}
				stack.pop();
				continue;
			}
			if name == "." {
				continue;
			}
			let last = pending.is_empty();
			let flags = libc::O_NOFOLLOW
				| libc::O_NOCTTY
				| if last {
					libc::O_RDONLY | libc::O_NONBLOCK
				} else {
					libc::O_RDONLY | libc::O_DIRECTORY
				};
			let dir = stack.last().expect("stack holds the root");
			let opened = match open_at(Some(dir), &name, flags) {
				Ok(fd) => fd,
				Err(err)
					if matches!(err.raw_os_error(), Some(libc::ELOOP) | Some(libc::ENOTDIR)) =>
				{
					let Some(target) = read_link(dir, &name)? else {
						return Err(err);
					};
					links += 1;
					if links > MAX_LINKS {
						return Err(io::Error::from_raw_os_error(libc::ELOOP));
					}
					let relative = if target.is_absolute() {
						let inner = target.strip_prefix(real_root).map_err(|_| escaped())?;
						stack.truncate(1);
						inner.to_path_buf()
					} else {
						target
					};
					for component in relative.components().rev() {
						match component {
							Component::Normal(part) => pending.push(part.to_os_string()),
							Component::ParentDir => pending.push("..".into()),
							Component::CurDir | Component::RootDir | Component::Prefix(_) => {}
						}
					}
					continue;
				}
				Err(err) => return Err(err),
			};
			if last {
				return Ok(File::from(opened));
			}
			stack.push(opened);
		}
		// Every component was consumed by links ending at a directory.
		Ok(File::from(stack.pop().expect("stack holds the root")))
	}

	/// The link's target when `name` is a symlink, or `None` when the open
	/// failed for another reason.
	fn read_link(dir: &OwnedFd, name: &OsStr) -> io::Result<Option<PathBuf>> {
		let c_name = cstring(name)?;
		let mut buf = vec![0u8; libc::PATH_MAX as usize];
		let len = unsafe {
			libc::readlinkat(
				dir.as_raw_fd(),
				c_name.as_ptr(),
				buf.as_mut_ptr() as *mut libc::c_char,
				buf.len(),
			)
		};
		if len < 0 {
			let err = io::Error::last_os_error();
			return if err.raw_os_error() == Some(libc::EINVAL) {
				Ok(None)
			} else {
				Err(err)
			};
		}
		buf.truncate(len as usize);
		Ok(Some(PathBuf::from(OsStr::from_bytes(&buf))))
	}

	/// The walk already refuses every link that leaves the root; asking the
	/// kernel for the handle's path is a second check against firmlink and
	/// mount surprises the walk cannot see.
	#[cfg(target_os = "macos")]
	fn check_final_path(root: &OwnedFd, file: &File) -> io::Result<()> {
		fn real_path(fd: libc::c_int) -> io::Result<PathBuf> {
			let mut buf = vec![0u8; libc::MAXPATHLEN as usize];
			if unsafe { libc::fcntl(fd, libc::F_GETPATH, buf.as_mut_ptr()) } < 0 {
				return Err(io::Error::last_os_error());
			}
			let len = buf.iter().position(|b| *b == 0).unwrap_or(buf.len());
			buf.truncate(len);
			Ok(PathBuf::from(OsStr::from_bytes(&buf)))
		}
		if real_path(file.as_raw_fd())?.starts_with(real_path(root.as_raw_fd())?) {
			Ok(())
		} else {
			Err(escaped())
		}
	}
}

#[cfg(windows)]
mod imp {
	use std::ffi::OsString;
	use std::fs::{File, OpenOptions};
	use std::io;
	use std::os::windows::ffi::OsStringExt;
	use std::os::windows::fs::OpenOptionsExt;
	use std::os::windows::io::AsRawHandle;
	use std::path::{Path, PathBuf};

	use windows_sys::Win32::Storage::FileSystem::{
		GetFinalPathNameByHandleW, FILE_FLAG_BACKUP_SEMANTICS, FILE_NAME_NORMALIZED,
	};

	use super::escaped;

	/// Backup semantics is what lets Windows open a directory handle.
	pub fn open_root(path: &Path) -> io::Result<File> {
		OpenOptions::new()
			.read(true)
			.custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
			.open(path)
	}

	pub fn open_beneath(root: &Path, components: &[OsString]) -> io::Result<File> {
		let root_handle = open_root(root)?;
		let file = open_root(&components.iter().fold(root.to_path_buf(), |p, c| p.join(c)))?;
		// Both sides come from the same OS call, so case, 8.3 names and
		// junctions in the root's own spelling compare equal.
		if final_path(&file)?.starts_with(final_path(&root_handle)?) {
			Ok(file)
		} else {
			Err(escaped())
		}
	}

	fn final_path(file: &File) -> io::Result<PathBuf> {
		let mut buf = vec![0u16; 1024];
		loop {
			let len = unsafe {
				GetFinalPathNameByHandleW(
					file.as_raw_handle() as _,
					buf.as_mut_ptr(),
					buf.len() as u32,
					FILE_NAME_NORMALIZED,
				)
			} as usize;
			if len == 0 {
				return Err(io::Error::last_os_error());
			}
			if len < buf.len() {
				buf.truncate(len);
				return Ok(PathBuf::from(OsString::from_wide(&buf)));
			}
			buf.resize(len + 1, 0);
		}
	}
}

#[cfg(all(test, unix))]
mod tests {
	use std::io::Read;
	use std::os::unix::fs::symlink;

	use super::*;

	fn tree() -> (tempfile::TempDir, std::path::PathBuf) {
		let temp = tempfile::tempdir().expect("tempdir");
		let root = temp.path().join("root");
		std::fs::create_dir_all(root.join("a/b")).unwrap();
		std::fs::write(root.join("a/b/inner.txt"), b"inner").unwrap();
		std::fs::create_dir_all(temp.path().join("outside/sub")).unwrap();
		std::fs::write(temp.path().join("outside/secret.txt"), b"secret").unwrap();
		std::fs::write(temp.path().join("outside/sub/deep.txt"), b"deep").unwrap();
		(temp, root)
	}

	fn read(root: &Path, relative: &str) -> io::Result<String> {
		let mut out = String::new();
		open_beneath(root, Path::new(relative))?.read_to_string(&mut out)?;
		Ok(out)
	}

	#[test]
	fn a_nested_file_opens() {
		let (_temp, root) = tree();
		assert_eq!(read(&root, "a/b/inner.txt").unwrap(), "inner");
		assert!(open_beneath(&root, Path::new("a/b"))
			.unwrap()
			.metadata()
			.unwrap()
			.is_dir());
	}

	#[test]
	fn traversal_and_absolute_paths_are_refused_before_opening() {
		let (temp, root) = tree();
		for bad in [
			"../outside/secret.txt",
			"a/../../outside/secret.txt",
			"a/./b/inner.txt",
			"a//b/inner.txt",
			"a/b/",
			"",
		] {
			let err = open_beneath(&root, Path::new(bad)).unwrap_err();
			assert_eq!(err.kind(), io::ErrorKind::InvalidInput, "{bad}");
		}
		let absolute = temp.path().join("outside/secret.txt");
		let err = open_beneath(&root, &absolute).unwrap_err();
		assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
		let long = "x".repeat(256);
		let err = open_beneath(&root, Path::new(&long)).unwrap_err();
		assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
	}

	#[test]
	fn a_file_symlink_out_of_the_root_is_refused() {
		let (temp, root) = tree();
		symlink(temp.path().join("outside/secret.txt"), root.join("abs")).unwrap();
		symlink("../outside/secret.txt", root.join("rel")).unwrap();
		for link in ["abs", "rel"] {
			let err = read(&root, link).unwrap_err();
			assert_eq!(err.kind(), io::ErrorKind::PermissionDenied, "{link}");
		}
	}

	#[test]
	fn a_directory_symlink_out_of_the_root_is_refused() {
		let (temp, root) = tree();
		symlink(temp.path().join("outside"), root.join("a/out")).unwrap();
		let err = read(&root, "a/out/sub/deep.txt").unwrap_err();
		assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
		let err = open_beneath(&root, Path::new("a/out")).unwrap_err();
		assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
	}

	#[test]
	fn a_symlink_that_stays_inside_the_root_opens() {
		let (_temp, root) = tree();
		symlink("b/inner.txt", root.join("a/file_link")).unwrap();
		symlink("../a/b", root.join("a/dir_link")).unwrap();
		symlink(root.join("a/b/inner.txt"), root.join("absolute_inside")).unwrap();
		assert_eq!(read(&root, "a/file_link").unwrap(), "inner");
		assert_eq!(read(&root, "a/dir_link/inner.txt").unwrap(), "inner");
		assert_eq!(read(&root, "absolute_inside").unwrap(), "inner");
	}

	#[test]
	fn a_symlinked_root_still_contains() {
		let (temp, root) = tree();
		let alias = temp.path().join("alias");
		symlink(&root, &alias).unwrap();
		symlink(root.join("a/b/inner.txt"), root.join("absolute_inside")).unwrap();
		assert_eq!(read(&alias, "a/b/inner.txt").unwrap(), "inner");
		assert_eq!(read(&alias, "absolute_inside").unwrap(), "inner");
		symlink(temp.path().join("outside/secret.txt"), root.join("abs")).unwrap();
		let err = read(&alias, "abs").unwrap_err();
		assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
	}

	#[test]
	fn a_fifo_opens_without_waiting_for_a_writer() {
		let (_temp, root) = tree();
		let fifo = std::ffi::CString::new(root.join("pipe").into_os_string().into_encoded_bytes())
			.unwrap();
		assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
		let meta = open_beneath(&root, Path::new("pipe"))
			.unwrap()
			.metadata()
			.unwrap();
		assert!(std::os::unix::fs::FileTypeExt::is_fifo(&meta.file_type()));
	}

	#[test]
	fn the_walk_alone_contains() {
		let (temp, root) = tree();
		symlink(temp.path().join("outside"), root.join("a/out")).unwrap();
		symlink("../a/b", root.join("a/dir_link")).unwrap();
		let real = std::fs::canonicalize(&root).unwrap();
		let walk = |rel: &str| -> io::Result<String> {
			let fd: std::os::fd::OwnedFd = File::open(&real)?.into();
			let mut out = String::new();
			imp::walk(&fd, &real, &validate(Path::new(rel))?)?.read_to_string(&mut out)?;
			Ok(out)
		};
		assert_eq!(walk("a/dir_link/inner.txt").unwrap(), "inner");
		assert_eq!(
			walk("a/out/sub/deep.txt").unwrap_err().kind(),
			io::ErrorKind::PermissionDenied
		);
	}
}
