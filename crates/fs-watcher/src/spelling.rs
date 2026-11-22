//! Translating between the spelling a watch is stored under and the spellings
//! the platform will actually deliver events for.
//!
//! macOS reaches the writable half of the disk twice. `/Users/me` and
//! `/System/Volumes/Data/Users/me` are the same directory: same device, same
//! inode, joined by a firmlink. `realpath` does not resolve firmlinks, so both
//! spellings survive canonicalization, and FSEvents subscribes to exactly one
//! of them. Registered the long way round it accepts the watch, reports no
//! error, and delivers nothing.
//!
//! The mapping is not guesswork: `/usr/share/firmlinks` publishes it, one line
//! per link, root-volume path and Data-volume-relative path separated by a tab.
//!
//! Everything above this module works in one spelling — the one the source
//! stores, which is the long one, because a whole-drive source is rooted at
//! `/System/Volumes/Data` and its records have to sit under their own root.
//! [`subscriptions`] translates outbound, at registration, and [`restore`]
//! translates back inbound, so a path that leaves in FSEvents' spelling returns
//! in Spacedrive's.

use std::path::{Path, PathBuf};

/// One platform subscription backing a watch.
///
/// A watch is usually one of these. It is more than one when the requested root
/// spans several firmlinks, which is the whole-drive case: `/System/Volumes/Data`
/// has no single short spelling, so it fans out into the links beneath it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subscription {
	/// The path handed to the platform watcher.
	pub subscribe: PathBuf,
	/// The prefix an event arriving under `subscribe` is rewritten back to.
	pub restore_to: PathBuf,
}

impl Subscription {
	/// A subscription that needs no translation.
	fn direct(path: &Path) -> Self {
		Self {
			subscribe: path.to_path_buf(),
			restore_to: path.to_path_buf(),
		}
	}
}

/// Rewrite an event path from the spelling it arrived in to the spelling the
/// watch is stored under.
///
/// Returns `None` when the path is not under this subscription, which is the
/// ordinary answer while looking for the one that owns it.
pub fn restore(subscription: &Subscription, path: &Path) -> Option<PathBuf> {
	let rest = path.strip_prefix(&subscription.subscribe).ok()?;
	Some(subscription.restore_to.join(rest))
}

/// The platform subscriptions a watch on `path` requires.
///
/// Never empty: a path with no firmlink translation subscribes to itself.
pub fn subscriptions(path: &Path) -> Vec<Subscription> {
	#[cfg(target_os = "macos")]
	{
		macos::subscriptions(path)
	}
	#[cfg(not(target_os = "macos"))]
	{
		vec![Subscription::direct(path)]
	}
}

/// The spelling of `path` a person would type.
///
/// On macOS a path under `/System/Volumes/Data` that a firmlink also reaches
/// is written through the link, so `/System/Volumes/Data/Users/me` reads as
/// `/Users/me`, the spelling a source is almost always tracked under. Every
/// other path, and every path on other platforms, is returned as given.
pub fn short_spelling(path: &Path) -> PathBuf {
	#[cfg(target_os = "macos")]
	{
		macos::short_spelling(path)
	}
	#[cfg(not(target_os = "macos"))]
	{
		path.to_path_buf()
	}
}

/// [`short_spelling`] over a given firmlink table, so the rewrite can be
/// checked where there are no firmlinks. `links` pairs each link's path on
/// the root volume with its path relative to the Data volume.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn shorten_with<'a>(
	path: &Path,
	data_volume: &Path,
	links: impl IntoIterator<Item = (&'a Path, &'a Path)>,
	same_directory: impl Fn(&Path, &Path) -> bool,
) -> Option<PathBuf> {
	let rest = path.strip_prefix(data_volume).ok()?;
	for (root, relative) in links {
		let Ok(below) = rest.strip_prefix(relative) else {
			continue;
		};
		if same_directory(root, &data_volume.join(relative)) {
			return Some(if below.as_os_str().is_empty() {
				root.to_path_buf()
			} else {
				root.join(below)
			});
		}
	}
	None
}

/// Whether `path` names an entry under exactly this spelling.
///
/// A case-insensitive volume (APFS and NTFS by default, ext4 with casefold)
/// answers a stat for any case of a name, so after `Readme.md` becomes
/// `README.md` the old spelling still looks present and its record would never
/// leave. Presence is decided by the name the directory itself holds. On a
/// case-sensitive volume this agrees with a stat, so `Readme.md` and
/// `README.md` stay two files.
pub fn present_as_spelled(path: &Path) -> std::io::Result<bool> {
	present_with(
		path,
		|path| std::fs::symlink_metadata(path).map(drop),
		stored_names,
	)
}

/// [`present_as_spelled`] with the stat and the directory read passed in, so
/// a case-insensitive volume can be stood in for where there is none.
fn present_with(
	path: &Path,
	stat: impl FnOnce(&Path) -> std::io::Result<()>,
	stored: impl FnOnce(&Path) -> std::io::Result<Vec<std::ffi::OsString>>,
) -> std::io::Result<bool> {
	match stat(path) {
		Ok(()) => {}
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
		Err(error) => return Err(error),
	}
	let Some(name) = path.file_name() else {
		return Ok(true);
	};
	Ok(stored(path)?.iter().any(|stored| stored == name))
}

/// The names the directory holding `path` stores for it.
///
/// macOS reads back the one stored name for the path in a single call. Other
/// platforms have no such call, so the parent is listed; the callers that run
/// on every event are macOS-only for that reason.
fn stored_names(path: &Path) -> std::io::Result<Vec<std::ffi::OsString>> {
	#[cfg(target_os = "macos")]
	if let Ok(name) = macos::stored_name(path) {
		return Ok(vec![name]);
	}
	let parent = match path.parent() {
		Some(parent) if !parent.as_os_str().is_empty() => parent,
		_ => Path::new("."),
	};
	std::fs::read_dir(parent)?
		.map(|entry| entry.map(|entry| entry.file_name()))
		.collect()
}

#[cfg(target_os = "macos")]
mod macos {
	use super::Subscription;
	use std::path::{Path, PathBuf};
	use std::sync::OnceLock;

	/// Where the Data volume mounts, and the root of every long spelling.
	const DATA_VOLUME: &str = "/System/Volumes/Data";
	const FIRMLINKS: &str = "/usr/share/firmlinks";

	/// The name the volume stores for the last component of `path`, read
	/// with `getattrlist`, which resolves the path the way a stat does and
	/// answers with the catalog's spelling rather than the caller's.
	pub(super) fn stored_name(path: &Path) -> std::io::Result<std::ffi::OsString> {
		use std::os::unix::ffi::{OsStrExt, OsStringExt};

		let c_path = std::ffi::CString::new(path.as_os_str().as_bytes())?;
		let mut request: libc::attrlist = unsafe { std::mem::zeroed() };
		request.bitmapcount = libc::ATTR_BIT_MAP_COUNT;
		request.commonattr = libc::ATTR_CMN_NAME;
		// A u32 length, an attrreference_t, then up to NAME_MAX UTF-8 characters.
		let mut buffer = [0u8; 4 + 8 + 1024];
		let result = unsafe {
			libc::getattrlist(
				c_path.as_ptr(),
				(&mut request as *mut libc::attrlist).cast(),
				buffer.as_mut_ptr().cast(),
				buffer.len(),
				libc::FSOPT_NOFOLLOW,
			)
		};
		if result != 0 {
			return Err(std::io::Error::last_os_error());
		}
		let field = |at: usize| [buffer[at], buffer[at + 1], buffer[at + 2], buffer[at + 3]];
		// The offset counts from the attrreference_t itself, which sits after the length.
		let offset = i32::from_ne_bytes(field(4));
		let length = u32::from_ne_bytes(field(8)) as usize;
		let start = usize::try_from(4 + i64::from(offset))
			.map_err(|_| std::io::Error::other("getattrlist name offset out of range"))?;
		let name = buffer
			.get(start..start + length)
			.ok_or_else(|| std::io::Error::other("getattrlist name out of range"))?;
		let name = name.strip_suffix(&[0]).unwrap_or(name);
		Ok(std::ffi::OsString::from_vec(name.to_vec()))
	}

	/// One published firmlink: a path on the root volume, and the same
	/// directory's path relative to the Data volume root.
	struct Firmlink {
		root: PathBuf,
		relative: PathBuf,
	}

	fn table() -> &'static [Firmlink] {
		static TABLE: OnceLock<Vec<Firmlink>> = OnceLock::new();
		TABLE.get_or_init(|| {
			let Ok(contents) = std::fs::read_to_string(FIRMLINKS) else {
				return Vec::new();
			};
			contents
				.lines()
				.filter_map(|line| {
					let (root, relative) = line.split_once('\t')?;
					if root.is_empty() || relative.is_empty() {
						return None;
					}
					Some(Firmlink {
						root: PathBuf::from(root),
						relative: PathBuf::from(relative),
					})
				})
				.collect()
		})
	}

	/// Whether two paths are the same directory, so a rewrite can never land
	/// somewhere else because the published table disagreed with the disk.
	fn same_directory(a: &Path, b: &Path) -> bool {
		use std::os::unix::fs::MetadataExt;
		match (std::fs::metadata(a), std::fs::metadata(b)) {
			(Ok(a), Ok(b)) => a.dev() == b.dev() && a.ino() == b.ino(),
			_ => false,
		}
	}

	pub(super) fn short_spelling(path: &Path) -> PathBuf {
		super::shorten_with(
			path,
			Path::new(DATA_VOLUME),
			table()
				.iter()
				.map(|link| (link.root.as_path(), link.relative.as_path())),
			same_directory,
		)
		.unwrap_or_else(|| path.to_path_buf())
	}

	pub(super) fn subscriptions(path: &Path) -> Vec<Subscription> {
		let Ok(rest) = path.strip_prefix(DATA_VOLUME) else {
			return vec![Subscription::direct(path)];
		};

		let mut found = Vec::new();

		for link in table() {
			// The requested root sits inside this firmlink: one subscription,
			// the short spelling of the same directory.
			if let Ok(below) = rest.strip_prefix(&link.relative) {
				let subscribe = link.root.join(below);
				if same_directory(&subscribe, path) {
					return vec![Subscription {
						subscribe,
						restore_to: path.to_path_buf(),
					}];
				}
				continue;
			}

			// The firmlink sits inside the requested root: part of a fan-out.
			if link.relative.starts_with(rest) {
				let restore_to = Path::new(DATA_VOLUME).join(&link.relative);
				if same_directory(&link.root, &restore_to) {
					found.push(Subscription {
						subscribe: link.root.clone(),
						restore_to,
					});
				}
			}
		}

		if found.is_empty() {
			// A Data-volume path outside every firmlink is only reachable the
			// long way round, so watch it as asked.
			return vec![Subscription::direct(path)];
		}

		found
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	/// A volume that answers a stat for any case of a name and lists each
	/// entry under the one spelling it stores.
	fn folding(
		stored: &'static [&'static str],
	) -> (
		impl Fn(&Path) -> std::io::Result<()>,
		impl Fn(&Path) -> std::io::Result<Vec<std::ffi::OsString>>,
	) {
		let stat = move |path: &Path| {
			let name = path.file_name().unwrap().to_string_lossy().to_lowercase();
			if stored.iter().any(|stored| stored.to_lowercase() == name) {
				Ok(())
			} else {
				Err(std::io::ErrorKind::NotFound.into())
			}
		};
		let list = move |_: &Path| Ok(stored.iter().map(Into::into).collect());
		(stat, list)
	}

	/// The same volume, case-sensitive.
	fn exact(
		stored: &'static [&'static str],
	) -> (
		impl Fn(&Path) -> std::io::Result<()>,
		impl Fn(&Path) -> std::io::Result<Vec<std::ffi::OsString>>,
	) {
		let stat = move |path: &Path| {
			let name = path.file_name().unwrap().to_string_lossy().into_owned();
			if stored.contains(&name.as_str()) {
				Ok(())
			} else {
				Err(std::io::ErrorKind::NotFound.into())
			}
		};
		let list = move |_: &Path| Ok(stored.iter().map(Into::into).collect());
		(stat, list)
	}

	const DATA: &str = "/System/Volumes/Data";

	fn links() -> Vec<(&'static Path, &'static Path)> {
		vec![
			(Path::new("/Users"), Path::new("Users")),
			(Path::new("/Applications"), Path::new("Applications")),
		]
	}

	#[test]
	fn a_data_volume_path_reads_through_its_firmlink() {
		let short = shorten_with(
			Path::new("/System/Volumes/Data/Users/me/Notes/a.txt"),
			Path::new(DATA),
			links(),
			|_, _| true,
		);
		assert_eq!(short, Some(PathBuf::from("/Users/me/Notes/a.txt")));
		assert_eq!(
			shorten_with(
				Path::new("/System/Volumes/Data/Users"),
				Path::new(DATA),
				links(),
				|_, _| true
			),
			Some(PathBuf::from("/Users"))
		);
	}

	#[test]
	fn a_path_no_firmlink_reaches_keeps_its_spelling() {
		for path in [
			"/System/Volumes/Data/private/var/db",
			"/System/Volumes/Data/UsersArchive/a.txt",
			"/Volumes/Archive/Users/me",
			"/Users/me/a.txt",
		] {
			assert_eq!(
				shorten_with(Path::new(path), Path::new(DATA), links(), |_, _| true),
				None,
				"{path}"
			);
		}
	}

	#[test]
	fn a_firmlink_the_disk_disagrees_with_is_not_followed() {
		assert_eq!(
			shorten_with(
				Path::new("/System/Volumes/Data/Users/me"),
				Path::new(DATA),
				links(),
				|_, _| false
			),
			None
		);
	}

	#[test]
	fn a_case_only_rename_leaves_the_old_spelling_absent_on_a_folding_volume() {
		let (stat, list) = folding(&["README.md"]);
		let old = Path::new("/vol/Readme.md");
		assert!(stat(old).is_ok(), "the stand-in folds case like APFS");
		assert!(!present_with(old, &stat, &list).unwrap());
		assert!(present_with(Path::new("/vol/README.md"), &stat, &list).unwrap());
	}

	#[test]
	fn two_spellings_stay_two_files_on_a_case_sensitive_volume() {
		let (stat, list) = exact(&["Readme.md", "README.md"]);
		assert!(present_with(Path::new("/vol/Readme.md"), &stat, &list).unwrap());
		assert!(present_with(Path::new("/vol/README.md"), &stat, &list).unwrap());
		assert!(!present_with(Path::new("/vol/readme.md"), &stat, &list).unwrap());
	}

	#[test]
	fn an_absent_path_is_not_listed() {
		let listed = std::cell::Cell::new(false);
		let present = present_with(
			Path::new("/vol/gone.txt"),
			|_| Err(std::io::ErrorKind::NotFound.into()),
			|_| {
				listed.set(true);
				Ok(Vec::new())
			},
		)
		.unwrap();
		assert!(!present);
		assert!(
			!listed.get(),
			"a missing path costs a stat and nothing else"
		);
	}

	#[test]
	fn an_unreadable_path_is_an_error_rather_than_absent() {
		let answer = present_with(
			Path::new("/vol/locked.txt"),
			|_| Err(std::io::ErrorKind::PermissionDenied.into()),
			|_| Ok(Vec::new()),
		);
		assert!(answer.is_err());
	}

	#[test]
	fn the_real_filesystem_agrees_with_itself() {
		let dir = tempfile::tempdir().unwrap();
		let path = dir.path().join("Readme.md");
		std::fs::write(&path, b"x").unwrap();
		assert!(present_as_spelled(&path).unwrap());
		assert!(!present_as_spelled(&dir.path().join("missing.md")).unwrap());
		std::fs::rename(&path, dir.path().join("README.md")).unwrap();
		assert!(!present_as_spelled(&path).unwrap());
		assert!(present_as_spelled(&dir.path().join("README.md")).unwrap());
	}

	#[test]
	fn a_path_with_no_translation_watches_itself() {
		let path = Path::new("/Volumes/Archive/Photos");
		assert_eq!(subscriptions(path), vec![Subscription::direct(path)]);
	}

	#[test]
	fn an_event_is_rewritten_into_the_stored_spelling() {
		let subscription = Subscription {
			subscribe: PathBuf::from("/Users/me/Desktop"),
			restore_to: PathBuf::from("/System/Volumes/Data/Users/me/Desktop"),
		};

		assert_eq!(
			restore(&subscription, Path::new("/Users/me/Desktop/shot.png")),
			Some(PathBuf::from(
				"/System/Volumes/Data/Users/me/Desktop/shot.png"
			))
		);
		assert_eq!(
			restore(&subscription, Path::new("/Users/me/Desktop")),
			Some(PathBuf::from("/System/Volumes/Data/Users/me/Desktop"))
		);
	}

	#[test]
	fn an_event_outside_a_subscription_is_not_claimed() {
		let subscription = Subscription {
			subscribe: PathBuf::from("/Users/me/Desktop"),
			restore_to: PathBuf::from("/System/Volumes/Data/Users/me/Desktop"),
		};

		assert_eq!(
			restore(&subscription, Path::new("/Users/me/Documents/notes.txt")),
			None
		);
	}

	/// A path that merely begins with the same bytes is a different directory
	/// and must not be rewritten into one.
	#[test]
	fn a_lookalike_prefix_is_left_alone() {
		let impostor = Path::new("/System/Volumes/Data-not-really/Users");
		assert_eq!(
			subscriptions(impostor),
			vec![Subscription::direct(impostor)]
		);
	}

	#[cfg(target_os = "macos")]
	#[test]
	fn a_long_spelling_subscribes_to_the_short_one() {
		let long = Path::new("/System/Volumes/Data/Users");
		let subs = subscriptions(long);

		assert_eq!(subs.len(), 1, "one directory, one subscription");
		assert_eq!(subs[0].subscribe, Path::new("/Users"));
		assert_eq!(subs[0].restore_to, long);
	}

	/// The whole Data volume has no short spelling of its own, so a watch on it
	/// only reaches the disk by subscribing to each firmlink beneath it.
	#[cfg(target_os = "macos")]
	#[test]
	fn the_whole_data_volume_fans_out_into_its_firmlinks() {
		let subs = subscriptions(Path::new("/System/Volumes/Data"));

		assert!(
			subs.len() > 1,
			"expected a fan-out, got {:?}",
			subs.iter().map(|s| &s.subscribe).collect::<Vec<_>>()
		);
		assert!(
			subs.iter().any(|s| s.subscribe == Path::new("/Users")),
			"the home directories are the point of this watch"
		);
		for subscription in &subs {
			assert!(
				subscription.restore_to.starts_with("/System/Volumes/Data"),
				"{:?} would deliver events outside the watch",
				subscription
			);
		}
	}

	/// The round trip is what the watcher relies on: subscribe under one
	/// spelling, receive under it, hand back the other.
	#[cfg(target_os = "macos")]
	#[test]
	fn a_fanned_out_event_returns_in_the_stored_spelling() {
		let subs = subscriptions(Path::new("/System/Volumes/Data"));
		let users = subs
			.iter()
			.find(|s| s.subscribe == Path::new("/Users"))
			.expect("the /Users firmlink");

		assert_eq!(
			restore(users, Path::new("/Users/me/Desktop/shot.png")),
			Some(PathBuf::from(
				"/System/Volumes/Data/Users/me/Desktop/shot.png"
			))
		);
	}
}
