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
use std::time::SystemTime;

const MAX_COMPONENT_BYTES: usize = 255;

/// Opens `relative` read-only beneath `root`, refusing any resolution that
/// leaves it.
///
/// `relative` is validated before the filesystem is touched: absolute paths,
/// empty, `.` and `..` components, NUL and components over 255 bytes are
/// `InvalidInput`, as are, on Windows, `:`, a trailing dot or space and
/// reserved device names. Windows accepts `\` as a separator like `/`.
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

/// Confirms that `relative` resolves beneath `root` without opening it for
/// reading.
///
/// Links are followed the way [`open_beneath`] follows them, and the answer
/// is the same: `PermissionDenied` for a resolution that leaves the root,
/// `NotFound` when nothing is there. Nothing is opened for reading, so a
/// FIFO cannot block the call and a cloud placeholder is not downloaded. Linux
/// opens the path with `O_PATH`; elsewhere the last component is only
/// stat'ed.
pub fn reach_beneath(root: &Path, relative: &Path) -> io::Result<()> {
	let components = validate(relative)?;
	imp::reach_beneath(root, &components)
}

/// Describes what `relative` beneath `root` resolves to, following links
/// that stay beneath it, without opening it for reading.
///
/// It answers what [`open_beneath`] would open, refused the same way, but
/// a cloud placeholder is not downloaded and a FIFO cannot block. An empty
/// `relative` describes the root itself.
pub fn metadata_beneath(root: &Path, relative: &Path) -> io::Result<Stat> {
	if relative.as_os_str().is_empty() {
		return Ok(Stat::from(&open_root(root)?.metadata()?));
	}
	imp::metadata_beneath(root, &validate(relative)?)
}

/// Describes `relative` beneath `root` without following a final symlink.
///
/// The parent directory is opened beneath the root and the last component is
/// described through that handle, so a link at the end is reported as a link
/// and a file is never opened, which keeps FIFOs and cloud placeholders safe.
/// An empty `relative` describes the root itself.
pub fn stat_beneath(root: &Path, relative: &Path) -> io::Result<Stat> {
	if relative.as_os_str().is_empty() {
		return Ok(Stat::from(&open_root(root)?.metadata()?));
	}
	let mut components = validate(relative)?;
	let leaf = components
		.pop()
		.expect("validate returns at least one component");
	Directory::open_components(root, &components)?.stat(&leaf)
}

/// What a path is, as [`stat_beneath`] and [`Directory::stat`] describe it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
	File,
	Dir,
	Symlink,
	Other,
}

/// The facts a caller can show about one path, taken from a handle.
#[derive(Debug, Clone)]
pub struct Stat {
	pub kind: Kind,
	pub len: u64,
	pub modified: Option<SystemTime>,
	pub accessed: Option<SystemTime>,
	pub created: Option<SystemTime>,
	pub readonly: bool,
}

impl From<&std::fs::Metadata> for Stat {
	fn from(meta: &std::fs::Metadata) -> Self {
		let file_type = meta.file_type();
		Self {
			kind: if file_type.is_symlink() {
				Kind::Symlink
			} else if file_type.is_dir() {
				Kind::Dir
			} else if file_type.is_file() {
				Kind::File
			} else {
				Kind::Other
			},
			len: meta.len(),
			modified: meta.modified().ok(),
			accessed: meta.accessed().ok(),
			created: meta.created().ok(),
			readonly: meta.permissions().readonly(),
		}
	}
}

/// A directory opened beneath a root, listed and described through its
/// handle.
///
/// Listing a directory by path follows a symlink swapped in for it, so a
/// listing reads the names of whatever the path points at by then. Listing
/// through the handle reads the directory that was checked, and its entries
/// are described without following them.
pub struct Directory(imp::Directory);

impl Directory {
	/// Opens the directory at `relative` beneath `root`, or the root itself
	/// when `relative` is empty. Anything other than a directory is
	/// `NotADirectory`, refused before it is opened.
	pub fn open(root: &Path, relative: &Path) -> io::Result<Self> {
		if relative.as_os_str().is_empty() {
			return Self::open_components(root, &[]);
		}
		Self::open_components(root, &validate(relative)?)
	}

	fn open_components(root: &Path, components: &[std::ffi::OsString]) -> io::Result<Self> {
		imp::Directory::open(root, components).map(Self)
	}

	/// The names in the directory, unsorted, without `.` and `..`.
	pub fn names(&self) -> io::Result<Vec<std::ffi::OsString>> {
		self.0.names()
	}

	/// Describes the entry `name` without following it when it is a link.
	pub fn stat(&self, name: &std::ffi::OsStr) -> io::Result<Stat> {
		// Names come from listing the directory, so the length limit, which
		// counts UTF-8 bytes and not the UTF-16 units Windows limits, is not
		// applied; only names that could leave the directory are refused.
		let raw = name.as_encoded_bytes();
		let separator = raw.contains(&b'/') || (cfg!(windows) && raw.contains(&b'\\'));
		if matches!(raw, b"" | b"." | b"..") || raw.contains(&0) || separator {
			return Err(invalid("not a single name in the directory"));
		}
		self.0.stat(name)
	}
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
	use std::ffi::{CStr, CString, OsStr, OsString};
	use std::fs::File;
	use std::io;
	use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
	use std::os::unix::ffi::OsStrExt;
	use std::path::{Component, Path, PathBuf};

	use super::{escaped, Stat};

	/// Symlink expansions allowed in one walk, matching Linux's own limit.
	const MAX_LINKS: usize = 40;

	/// What the last component of a walk is opened as.
	#[derive(Clone, Copy, PartialEq, Eq)]
	pub(super) enum Leaf {
		/// Opened read-only and non-blocking, whatever it is.
		Any,
		/// Opened only if it is a directory.
		Directory,
		/// Stat'ed, never opened.
		Probe,
	}

	/// Where a walk ended.
	pub(super) enum Reached {
		Opened(File),
		/// A probe found the last component, by this name and not a link, in
		/// this directory.
		Probed(OwnedFd, OsString),
	}

	pub fn open_root(root: &Path) -> io::Result<File> {
		File::open(root)
	}

	pub fn open_beneath(root: &Path, components: &[OsString]) -> io::Result<File> {
		match resolve(root, components, Leaf::Any)? {
			Reached::Opened(file) => Ok(file),
			Reached::Probed(..) => unreachable!("only a probe stops short of opening"),
		}
	}

	pub fn reach_beneath(root: &Path, components: &[OsString]) -> io::Result<()> {
		resolve(root, components, Leaf::Probe).map(drop)
	}

	pub fn metadata_beneath(root: &Path, components: &[OsString]) -> io::Result<Stat> {
		match resolve(root, components, Leaf::Probe)? {
			Reached::Opened(file) => Ok(Stat::from(&file.metadata()?)),
			Reached::Probed(dir, name) => Directory(dir).stat(&name),
		}
	}

	/// Flags for a directory handle used only to resolve names beneath it,
	/// which needs search permission and not read permission, so a folder the
	/// daemon may traverse but not list still serves the files it names.
	#[cfg(target_os = "linux")]
	const SEARCH: libc::c_int = libc::O_PATH | libc::O_DIRECTORY;
	#[cfg(any(target_os = "macos", target_os = "ios"))]
	const SEARCH: libc::c_int = libc::O_SEARCH;
	#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "ios")))]
	const SEARCH: libc::c_int = libc::O_RDONLY | libc::O_DIRECTORY;

	/// The root's real path and a directory handle on it.
	pub(super) fn root_dir(root: &Path) -> io::Result<(PathBuf, OwnedFd)> {
		// The root is trusted, so resolve its spelling once; absolute links
		// are judged against this real path.
		let real_root = std::fs::canonicalize(root)?;
		let fd = open_at(None, real_root.as_os_str(), SEARCH)?;
		Ok((real_root, fd))
	}

	fn resolve(root: &Path, components: &[OsString], leaf: Leaf) -> io::Result<Reached> {
		let (real_root, root_fd) = root_dir(root)?;

		#[cfg(target_os = "linux")]
		match openat2_beneath(&root_fd, components, leaf) {
			Ok(file) => return Ok(Reached::Opened(file)),
			// ENOSYS is a kernel without openat2. EXDEV is a link leaving
			// the root or an absolute link, which the walk judges against
			// the real root so an absolute link back inside still opens.
			Err(err) if matches!(err.raw_os_error(), Some(libc::ENOSYS) | Some(libc::EXDEV)) => {}
			Err(err) => return Err(err),
		}

		let reached = walk_to(&root_fd, &real_root, components, leaf)?;
		#[cfg(target_os = "macos")]
		match &reached {
			Reached::Opened(file) => check_final_path(&root_fd, file.as_raw_fd())?,
			Reached::Probed(dir, _) => check_final_path(&root_fd, dir.as_raw_fd())?,
		}
		Ok(reached)
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

	/// `name` in `dir` as `fstatat` describes it, links not followed.
	fn lstat_at(dir: libc::c_int, name: &OsStr) -> io::Result<libc::stat> {
		let name = cstring(name)?;
		let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
		let result = unsafe {
			libc::fstatat(
				dir,
				name.as_ptr(),
				stat.as_mut_ptr(),
				libc::AT_SYMLINK_NOFOLLOW,
			)
		};
		if result != 0 {
			return Err(io::Error::last_os_error());
		}
		Ok(unsafe { stat.assume_init() })
	}

	#[cfg(target_os = "linux")]
	fn openat2_beneath(root: &OwnedFd, components: &[OsString], leaf: Leaf) -> io::Result<File> {
		let joined: PathBuf = components.iter().collect();
		let path = cstring(joined.as_os_str())?;
		let mut how: libc::open_how = unsafe { std::mem::zeroed() };
		let flags = match leaf {
			Leaf::Any => libc::O_RDONLY | libc::O_NOCTTY | libc::O_NONBLOCK,
			Leaf::Directory => libc::O_RDONLY | libc::O_DIRECTORY,
			Leaf::Probe => libc::O_PATH,
		};
		how.flags = (flags | libc::O_CLOEXEC) as u64;
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
	#[cfg(test)]
	pub(super) fn walk(
		root: &OwnedFd,
		real_root: &Path,
		components: &[OsString],
	) -> io::Result<File> {
		match walk_to(root, real_root, components, Leaf::Any)? {
			Reached::Opened(file) => Ok(file),
			Reached::Probed(..) => unreachable!("only a probe stops short of opening"),
		}
	}

	pub(super) fn walk_to(
		root: &OwnedFd,
		real_root: &Path,
		components: &[OsString],
		leaf: Leaf,
	) -> io::Result<Reached> {
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
			let dir = stack.last().expect("stack holds the root");
			let attempt = if last && leaf == Leaf::Probe {
				let stat = lstat_at(dir.as_raw_fd(), &name)?;
				if stat.st_mode & libc::S_IFMT != libc::S_IFLNK {
					return Ok(Reached::Probed(dir.try_clone()?, name));
				}
				Err(io::Error::from_raw_os_error(libc::ELOOP))
			} else {
				open_at(Some(dir), &name, libc::O_NOFOLLOW | leaf_flags(leaf, last))
			};
			let opened = match attempt {
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
						let inner = inside_root(root, real_root, &target)?;
						stack.truncate(1);
						inner
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
				return Ok(Reached::Opened(File::from(opened)));
			}
			stack.push(opened);
		}
		// Every component was consumed by links ending at a directory, whose
		// handle on the stack can only search, so it is opened again as asked.
		let dir = stack.pop().expect("stack holds the root");
		Ok(match leaf {
			Leaf::Probe => Reached::Probed(dir, ".".into()),
			Leaf::Any | Leaf::Directory => Reached::Opened(File::from(open_at(
				Some(&dir),
				OsStr::new("."),
				leaf_flags(leaf, true),
			)?)),
		})
	}

	/// An absolute link target relative to the root, when the root is one of
	/// its ancestors.
	///
	/// A link can spell the root another way than its real path, such as
	/// `/var` for `/private/var` on macOS or through any symlinked parent, so
	/// ancestors the prefix test misses are compared with the root's handle by
	/// device and inode. Only the prefix is chosen this way: the rest is still
	/// resolved from the root's handle, so a parent swapped during the check
	/// can only pick another path beneath the root.
	fn inside_root(root: &OwnedFd, real_root: &Path, target: &Path) -> io::Result<PathBuf> {
		use std::os::unix::fs::MetadataExt;

		if let Ok(inner) = target.strip_prefix(real_root) {
			return Ok(inner.to_path_buf());
		}
		let meta = File::from(root.try_clone()?).metadata()?;
		let (dev, ino) = (meta.dev(), meta.ino());
		target
			.ancestors()
			.find(|ancestor| {
				std::fs::metadata(ancestor).is_ok_and(|meta| meta.dev() == dev && meta.ino() == ino)
			})
			.and_then(|ancestor| target.strip_prefix(ancestor).ok())
			.map(Path::to_path_buf)
			.ok_or_else(escaped)
	}

	/// How the walk opens a component: the last as `leaf` asks, the rest as
	/// search-only directories.
	fn leaf_flags(leaf: Leaf, last: bool) -> libc::c_int {
		match (leaf, last) {
			(Leaf::Any, true) => libc::O_RDONLY | libc::O_NOCTTY | libc::O_NONBLOCK,
			(Leaf::Directory, true) => libc::O_RDONLY | libc::O_DIRECTORY,
			_ => SEARCH,
		}
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

	/// The kernel's own path for an open descriptor.
	#[cfg(target_os = "macos")]
	pub(super) fn kernel_path(fd: libc::c_int) -> io::Result<PathBuf> {
		let mut buf = vec![0u8; libc::MAXPATHLEN as usize];
		if unsafe { libc::fcntl(fd, libc::F_GETPATH, buf.as_mut_ptr()) } < 0 {
			return Err(io::Error::last_os_error());
		}
		let len = buf.iter().position(|b| *b == 0).unwrap_or(buf.len());
		buf.truncate(len);
		Ok(PathBuf::from(OsStr::from_bytes(&buf)))
	}

	/// The walk already refuses every link that leaves the root; asking the
	/// kernel for both handles' paths is a second check against firmlink and
	/// mount surprises the walk cannot see. Both paths come from `F_GETPATH`,
	/// so `/Users` and `/System/Volumes/Data/Users`, or two cases of one name
	/// on a case-insensitive volume, are spelled the same way on both sides
	/// whatever spelling the caller used for the root.
	#[cfg(target_os = "macos")]
	fn check_final_path(root: &OwnedFd, fd: libc::c_int) -> io::Result<()> {
		if kernel_path(fd)?.starts_with(kernel_path(root.as_raw_fd())?) {
			Ok(())
		} else {
			Err(escaped())
		}
	}

	#[cfg(target_os = "android")]
	use libc::__errno as errno;
	#[cfg(target_os = "linux")]
	use libc::__errno_location as errno;
	#[cfg(any(target_os = "macos", target_os = "ios", target_os = "freebsd"))]
	use libc::__error as errno;

	pub struct Directory(OwnedFd);

	impl Directory {
		pub fn open(root: &Path, components: &[OsString]) -> io::Result<Self> {
			if components.is_empty() {
				let real_root = std::fs::canonicalize(root)?;
				return open_at(
					None,
					real_root.as_os_str(),
					libc::O_RDONLY | libc::O_DIRECTORY,
				)
				.map(Self);
			}
			match resolve(root, components, Leaf::Directory)? {
				Reached::Opened(file) => Ok(Self(file.into())),
				Reached::Probed(..) => unreachable!("only a probe stops short of opening"),
			}
		}

		pub fn names(&self) -> io::Result<Vec<OsString>> {
			// fdopendir owns the descriptor it is given and closedir closes
			// it, so it gets a duplicate.
			let fd = unsafe { libc::fcntl(self.0.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 0) };
			if fd < 0 {
				return Err(io::Error::last_os_error());
			}
			let stream = unsafe { libc::fdopendir(fd) };
			if stream.is_null() {
				let err = io::Error::last_os_error();
				unsafe { libc::close(fd) };
				return Err(err);
			}
			// The duplicate shares the original's offset.
			unsafe { libc::rewinddir(stream) };
			let mut names = Vec::new();
			let result = loop {
				// readdir answers NULL both at the end and on error; only errno
				// tells them apart.
				unsafe { *errno() = 0 };
				let entry = unsafe { libc::readdir(stream) };
				if entry.is_null() {
					let errno = unsafe { *errno() };
					break if errno == 0 {
						Ok(())
					} else {
						Err(io::Error::from_raw_os_error(errno))
					};
				}
				let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
				if name != b"." && name != b".." {
					names.push(OsStr::from_bytes(name).to_os_string());
				}
			};
			unsafe { libc::closedir(stream) };
			result.map(|()| names)
		}

		/// Linux opens the entry with `O_PATH`, which reads nothing, so the
		/// description comes from the standard library and carries the birth
		/// time `statx` reports.
		#[cfg(target_os = "linux")]
		pub fn stat(&self, name: &OsStr) -> io::Result<Stat> {
			let fd = open_at(Some(&self.0), name, libc::O_PATH | libc::O_NOFOLLOW)?;
			Ok(Stat::from(&File::from(fd).metadata()?))
		}

		#[cfg(not(target_os = "linux"))]
		pub fn stat(&self, name: &OsStr) -> io::Result<Stat> {
			use std::time::{Duration, SystemTime};

			use super::Kind;

			let raw = lstat_at(self.0.as_raw_fd(), name)?;
			let time = |secs: libc::time_t, nanos: libc::c_long| {
				let secs = u64::try_from(secs).ok()?;
				SystemTime::UNIX_EPOCH.checked_add(Duration::new(secs, nanos as u32))
			};
			#[cfg(any(target_os = "macos", target_os = "ios"))]
			let created = time(raw.st_birthtime, raw.st_birthtime_nsec);
			#[cfg(not(any(target_os = "macos", target_os = "ios")))]
			let created = None;
			Ok(Stat {
				kind: match raw.st_mode & libc::S_IFMT {
					libc::S_IFLNK => Kind::Symlink,
					libc::S_IFDIR => Kind::Dir,
					libc::S_IFREG => Kind::File,
					_ => Kind::Other,
				},
				len: raw.st_size as u64,
				modified: time(raw.st_mtime, raw.st_mtime_nsec),
				accessed: time(raw.st_atime, raw.st_atime_nsec),
				created,
				readonly: raw.st_mode & 0o222 == 0,
			})
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

	pub fn reach_beneath(root: &Path, components: &[OsString]) -> io::Result<()> {
		open_beneath(root, components).map(drop)
	}

	pub fn metadata_beneath(root: &Path, components: &[OsString]) -> io::Result<super::Stat> {
		Ok(super::Stat::from(
			&open_beneath(root, components)?.metadata()?,
		))
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

	/// Windows cannot list or describe relative to a directory handle, so the
	/// handle is held open, which keeps the directory from being renamed or
	/// replaced, and its final path is used.
	pub struct Directory {
		path: PathBuf,
		_handle: File,
	}

	impl Directory {
		pub fn open(root: &Path, components: &[OsString]) -> io::Result<Self> {
			let handle = if components.is_empty() {
				open_root(root)?
			} else {
				open_beneath(root, components)?
			};
			if !handle.metadata()?.is_dir() {
				return Err(io::Error::from(io::ErrorKind::NotADirectory));
			}
			Ok(Self {
				path: final_path(&handle)?,
				_handle: handle,
			})
		}

		pub fn names(&self) -> io::Result<Vec<OsString>> {
			std::fs::read_dir(&self.path)?
				.map(|entry| entry.map(|entry| entry.file_name()))
				.collect()
		}

		pub fn stat(&self, name: &std::ffi::OsStr) -> io::Result<super::Stat> {
			Ok(super::Stat::from(&std::fs::symlink_metadata(
				self.path.join(name),
			)?))
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
	fn an_absolute_link_through_another_spelling_of_the_root_opens() {
		let (temp, _) = tree();
		let real = std::fs::canonicalize(temp.path()).unwrap();
		std::fs::create_dir_all(real.join("real/root/a")).unwrap();
		std::fs::write(real.join("real/root/a/inner.txt"), b"inner").unwrap();
		symlink(real.join("real"), real.join("alias")).unwrap();
		let root = real.join("alias/root");
		symlink(root.join("a/inner.txt"), root.join("aliased")).unwrap();
		symlink(real.join("outside/secret.txt"), root.join("escape")).unwrap();
		assert_eq!(read(&root, "aliased").unwrap(), "inner");
		assert_eq!(read(&real.join("real/root"), "aliased").unwrap(), "inner");
		assert_eq!(
			read(&root, "escape").unwrap_err().kind(),
			io::ErrorKind::PermissionDenied
		);
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

	#[test]
	fn reach_follows_inner_links_and_refuses_outer_ones() {
		let (temp, root) = tree();
		symlink("b/inner.txt", root.join("a/file_link")).unwrap();
		symlink(temp.path().join("outside"), root.join("a/out")).unwrap();
		assert!(reach_beneath(&root, Path::new("a/b/inner.txt")).is_ok());
		assert!(reach_beneath(&root, Path::new("a/file_link")).is_ok());
		for escape in ["a/out", "a/out/secret.txt"] {
			let err = reach_beneath(&root, Path::new(escape)).unwrap_err();
			assert_eq!(err.kind(), io::ErrorKind::PermissionDenied, "{escape}");
		}
		let err = reach_beneath(&root, Path::new("a/missing.txt")).unwrap_err();
		assert_eq!(err.kind(), io::ErrorKind::NotFound);
		let err = reach_beneath(&root, Path::new("../outside")).unwrap_err();
		assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
	}

	#[test]
	fn reach_does_not_wait_on_a_fifo() {
		let (_temp, root) = tree();
		let fifo = std::ffi::CString::new(root.join("pipe").into_os_string().into_encoded_bytes())
			.unwrap();
		assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
		assert!(reach_beneath(&root, Path::new("pipe")).is_ok());
		assert_eq!(
			stat_beneath(&root, Path::new("pipe")).unwrap().kind,
			Kind::Other
		);
	}

	#[test]
	fn stat_describes_a_final_link_without_following_it() {
		let (temp, root) = tree();
		symlink(
			temp.path().join("outside/secret.txt"),
			root.join("a/escape"),
		)
		.unwrap();
		let link = stat_beneath(&root, Path::new("a/escape")).unwrap();
		assert_eq!(link.kind, Kind::Symlink);
		let file = stat_beneath(&root, Path::new("a/b/inner.txt")).unwrap();
		assert_eq!((file.kind, file.len), (Kind::File, 5));
		assert!(file.modified.is_some());
		assert_eq!(stat_beneath(&root, Path::new("")).unwrap().kind, Kind::Dir);
		symlink(temp.path().join("outside"), root.join("out")).unwrap();
		let err = stat_beneath(&root, Path::new("out/secret.txt")).unwrap_err();
		assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
	}

	#[test]
	fn a_directory_lists_through_its_handle() {
		let (temp, root) = tree();
		symlink(temp.path().join("outside"), root.join("a/out")).unwrap();
		let dir = Directory::open(&root, Path::new("a")).unwrap();
		let mut names = dir.names().unwrap();
		names.sort();
		assert_eq!(names, vec![std::ffi::OsString::from("b"), "out".into()]);
		assert_eq!(dir.stat("b".as_ref()).unwrap().kind, Kind::Dir);
		assert_eq!(dir.stat("out".as_ref()).unwrap().kind, Kind::Symlink);
		assert_eq!(
			dir.stat("b/inner.txt".as_ref()).unwrap_err().kind(),
			io::ErrorKind::InvalidInput
		);
		let err = Directory::open(&root, Path::new("a/out")).err().unwrap();
		assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
		let err = Directory::open(&root, Path::new("a/b/inner.txt"))
			.err()
			.unwrap();
		assert_eq!(err.kind(), io::ErrorKind::NotADirectory);
		assert!(Directory::open(&root, Path::new(""))
			.unwrap()
			.names()
			.unwrap()
			.contains(&"a".into()));
	}

	/// Swaps `swap` between a real directory and a symlink to `outside`
	/// until `stop` is set. Each half is two renames, so the name is
	/// briefly missing too.
	fn swapper(
		root: std::path::PathBuf,
		stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
	) -> std::thread::JoinHandle<u64> {
		std::thread::spawn(move || {
			use std::sync::atomic::Ordering;
			let (name, aside, link) = (
				root.join("swap"),
				root.join("swap.real"),
				root.join("swap.link"),
			);
			let mut swaps = 0;
			while !stop.load(Ordering::Relaxed) {
				std::fs::rename(&name, &aside).unwrap();
				std::fs::rename(&link, &name).unwrap();
				std::fs::rename(&name, &link).unwrap();
				std::fs::rename(&aside, &name).unwrap();
				swaps += 1;
			}
			swaps
		})
	}

	#[test]
	fn metadata_follows_inner_links_without_opening() {
		let (temp, root) = tree();
		symlink("b/inner.txt", root.join("a/file_link")).unwrap();
		symlink("..", root.join("a/b/up")).unwrap();
		symlink(temp.path().join("outside"), root.join("a/out")).unwrap();
		let linked = metadata_beneath(&root, Path::new("a/file_link")).unwrap();
		assert_eq!((linked.kind, linked.len), (Kind::File, 5));
		assert_eq!(
			metadata_beneath(&root, Path::new("a/b/up")).unwrap().kind,
			Kind::Dir
		);
		assert_eq!(
			metadata_beneath(&root, Path::new("a/out/secret.txt"))
				.unwrap_err()
				.kind(),
			io::ErrorKind::PermissionDenied
		);
		assert_eq!(
			metadata_beneath(&root, Path::new("")).unwrap().kind,
			Kind::Dir
		);
		let fifo = std::ffi::CString::new(root.join("pipe").into_os_string().into_encoded_bytes())
			.unwrap();
		assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
		assert_eq!(
			metadata_beneath(&root, Path::new("pipe")).unwrap().kind,
			Kind::Other
		);
		let real = std::fs::canonicalize(&root).unwrap();
		let fd: std::os::fd::OwnedFd = File::open(&real).unwrap().into();
		let reached = imp::walk_to(
			&fd,
			&real,
			&validate(Path::new("a/b/up")).unwrap(),
			imp::Leaf::Directory,
		)
		.unwrap();
		let imp::Reached::Opened(dir) = reached else {
			panic!("a directory opens");
		};
		assert!(dir.metadata().unwrap().is_dir());
	}

	#[test]
	fn a_root_that_can_be_searched_but_not_listed_still_serves_named_files() {
		use std::os::unix::fs::PermissionsExt;

		if unsafe { libc::geteuid() } == 0 {
			// Root ignores directory permissions, so there is nothing to test.
			return;
		}
		let (_temp, root) = tree();
		let a = root.join("a");
		for dir in [&root, &a] {
			std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o111)).unwrap();
		}
		let read_result = read(&root, "a/b/inner.txt");
		let reached = reach_beneath(&root, Path::new("a/b/inner.txt"));
		let described = metadata_beneath(&root, Path::new("a/b/inner.txt"));
		let listed = Directory::open(&root, Path::new("a")).and_then(|dir| dir.names());
		let walked = imp::root_dir(&root)
			.and_then(|(real, fd)| imp::walk(&fd, &real, &validate(Path::new("a/b/inner.txt"))?));
		for dir in [&root, &a] {
			std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755)).unwrap();
		}
		assert_eq!(read_result.unwrap(), "inner");
		assert!(walked.is_ok(), "{walked:?}");
		assert!(reached.is_ok());
		assert_eq!(described.unwrap().kind, Kind::File);
		assert_eq!(listed.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
	}

	#[test]
	fn the_walk_alone_probes_and_opens_directories() {
		let (temp, root) = tree();
		symlink(temp.path().join("outside"), root.join("a/out")).unwrap();
		symlink("b/inner.txt", root.join("a/file_link")).unwrap();
		let real = std::fs::canonicalize(&root).unwrap();
		let walk = |rel: &str, leaf: imp::Leaf| -> io::Result<imp::Reached> {
			let fd: std::os::fd::OwnedFd = File::open(&real)?.into();
			imp::walk_to(&fd, &real, &validate(Path::new(rel))?, leaf)
		};
		assert!(matches!(
			walk("a/file_link", imp::Leaf::Probe).unwrap(),
			imp::Reached::Probed(..)
		));
		assert_eq!(
			walk("a/out", imp::Leaf::Probe).err().unwrap().kind(),
			io::ErrorKind::PermissionDenied
		);
		assert_eq!(
			walk("a/missing", imp::Leaf::Probe).err().unwrap().kind(),
			io::ErrorKind::NotFound
		);
		assert!(matches!(
			walk("a/b", imp::Leaf::Directory).unwrap(),
			imp::Reached::Opened(_)
		));
		assert_eq!(
			walk("a/file_link", imp::Leaf::Directory)
				.err()
				.unwrap()
				.kind(),
			io::ErrorKind::NotADirectory
		);
	}

	#[test]
	fn a_directory_swapped_for_an_outside_link_never_reads_outside() {
		use std::sync::atomic::{AtomicBool, Ordering};
		use std::sync::Arc;

		let (temp, root) = tree();
		std::fs::create_dir(root.join("swap")).unwrap();
		std::fs::write(root.join("swap/secret.txt"), b"inner").unwrap();
		symlink(temp.path().join("outside"), root.join("swap.link")).unwrap();
		let stop = Arc::new(AtomicBool::new(false));
		let swapper = swapper(root.clone(), stop.clone());

		let (mut inside, mut refused) = (0, 0);
		for _ in 0..5000 {
			match read(&root, "swap/secret.txt") {
				Ok(text) => {
					assert_eq!(text, "inner", "read outside the root");
					inside += 1;
				}
				Err(_) => refused += 1,
			}
			let real = std::fs::canonicalize(&root).unwrap();
			let fd: std::os::fd::OwnedFd = File::open(&real).unwrap().into();
			if let Ok(Ok(mut file)) =
				validate(Path::new("swap/secret.txt")).map(|parts| imp::walk(&fd, &real, &parts))
			{
				let mut text = String::new();
				if file.read_to_string(&mut text).is_ok() {
					assert_eq!(text, "inner", "the walk read outside the root");
				}
			}
			if let Ok(dir) = Directory::open(&root, Path::new("swap")) {
				let names = dir.names().unwrap();
				assert!(
					!names.iter().any(|name| name == "sub"),
					"listed outside the root: {names:?}"
				);
			}
		}
		stop.store(true, Ordering::Relaxed);
		let swaps = swapper.join().unwrap();
		assert!(
			swaps > 0 && inside > 0,
			"swaps {swaps}, inside {inside}, refused {refused}"
		);
	}
}

#[cfg(all(test, target_os = "macos"))]
mod macos_tests {
	use std::io::Read;
	use std::os::fd::AsRawFd;
	use std::os::unix::fs::symlink;
	use std::path::{Path, PathBuf};

	use super::*;

	fn tree() -> (tempfile::TempDir, PathBuf) {
		let temp = tempfile::tempdir().expect("tempdir");
		let root = std::fs::canonicalize(temp.path()).unwrap().join("Root");
		std::fs::create_dir_all(root.join("a")).unwrap();
		std::fs::write(root.join("a/inner.txt"), b"inner").unwrap();
		std::fs::create_dir_all(temp.path().join("outside")).unwrap();
		std::fs::write(temp.path().join("outside/secret.txt"), b"secret").unwrap();
		symlink(temp.path().join("outside/secret.txt"), root.join("escape")).unwrap();
		(temp, root)
	}

	fn read(root: &Path, relative: &str) -> io::Result<String> {
		let mut out = String::new();
		open_beneath(root, Path::new(relative))?.read_to_string(&mut out)?;
		Ok(out)
	}

	/// The same directory spelled through the data volume, as a firmlink
	/// reaches it.
	fn data_volume_spelling(path: &Path) -> Option<PathBuf> {
		let spelled = Path::new("/System/Volumes/Data").join(path.strip_prefix("/").ok()?);
		spelled.is_dir().then_some(spelled)
	}

	#[test]
	fn a_firmlink_spelling_of_the_root_contains() {
		let (_temp, root) = tree();
		let Some(spelled) = data_volume_spelling(&root) else {
			return;
		};
		assert_eq!(read(&spelled, "a/inner.txt").unwrap(), "inner");
		assert_eq!(
			read(&spelled, "escape").unwrap_err().kind(),
			io::ErrorKind::PermissionDenied
		);
		let plain = imp::kernel_path(open_root(&root).unwrap().as_raw_fd()).unwrap();
		let firm = imp::kernel_path(open_root(&spelled).unwrap().as_raw_fd()).unwrap();
		assert_eq!(plain, firm, "F_GETPATH spells both the same way");
	}

	#[test]
	fn a_case_alias_of_the_root_contains() {
		let (_temp, root) = tree();
		let alias = root.with_file_name("ROOT");
		if !alias.is_dir() {
			// A case-sensitive volume has no alias to test.
			return;
		}
		assert_eq!(read(&alias, "a/inner.txt").unwrap(), "inner");
		assert_eq!(read(&alias, "A/INNER.TXT").unwrap(), "inner");
		assert_eq!(
			read(&alias, "escape").unwrap_err().kind(),
			io::ErrorKind::PermissionDenied
		);
		assert!(reach_beneath(&alias, Path::new("A")).is_ok());
		assert_eq!(
			stat_beneath(&alias, Path::new("a/inner.txt")).unwrap().kind,
			Kind::File
		);
	}

	#[test]
	fn the_walk_lists_and_describes_on_macos() {
		let (_temp, root) = tree();
		let dir = Directory::open(&root, Path::new("a")).unwrap();
		assert_eq!(
			dir.names().unwrap(),
			vec![std::ffi::OsString::from("inner.txt")]
		);
		let stat = dir.stat("inner.txt".as_ref()).unwrap();
		assert_eq!((stat.kind, stat.len), (Kind::File, 5));
		assert!(stat.created.is_some());
		assert_eq!(
			stat_beneath(&root, Path::new("escape")).unwrap().kind,
			Kind::Symlink
		);
	}
}

#[cfg(all(test, windows))]
mod windows_tests {
	use std::path::Path;

	use super::*;

	#[test]
	fn a_listed_name_over_255_utf8_bytes_is_described() {
		let temp = tempfile::tempdir().expect("tempdir");
		let name = "\u{e9}".repeat(128);
		std::fs::write(temp.path().join(&name), b"x").unwrap();
		let dir = Directory::open(temp.path(), Path::new("")).unwrap();
		assert!(dir.names().unwrap().iter().any(|listed| *listed == *name));
		let stat = dir.stat(name.as_ref()).unwrap();
		assert_eq!((stat.kind, stat.len), (Kind::File, 1));
	}
}
