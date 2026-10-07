//! The trash, with where each item went.
//!
//! An item goes to the platform's trash where the volume has one, and the
//! location it lands at is recorded so it can be put back. On macOS that is
//! `NSFileManager`'s trash call, which answers with the resulting URL; on
//! Windows and Linux the `trash` crate lists the trash afterward and the item
//! is found by its original path. On Linux the location is the item itself
//! under the freedesktop `Trash/files` directory, and its `Trash/info`
//! companion is derived from that and goes when the item is restored or
//! purged. A volume with no trash of its own, a
//! network mount among them, gets a Spacedrive trash directory at its root,
//! `.spacedrive/trash/<job>/`, reached by a rename on the same volume.
//!
//! Replacing a file goes through the same door: the previous bytes are
//! trashed and their location recorded, so an overwrite is undoable until
//! the trash is emptied and costs a rename rather than a copy.

use std::{
	io,
	path::{Path, PathBuf},
};

use crate::{infra::job::types::JobId, volume::VolumeManager};

/// The directory beneath a volume's root that holds what Spacedrive trashed
/// there.
pub const SPACEDRIVE_TRASH: &str = ".spacedrive/trash";

/// Move `path` to the trash and answer with where it went, where the
/// platform says.
pub async fn trash(
	path: &Path,
	volumes: Option<&VolumeManager>,
	job: JobId,
) -> io::Result<Option<PathBuf>> {
	let owned = path.to_path_buf();
	let os = tokio::task::spawn_blocking(move || trash_os(&owned))
		.await
		.map_err(|error| io::Error::other(error.to_string()))?;
	match os {
		Ok(location) => Ok(location),
		Err(os_error) => match spacedrive_trash(path, volumes, job).await {
			Ok(location) => Ok(Some(location)),
			Err(_) => Err(os_error),
		},
	}
}

/// Put an item back where it was, from the location the trash reported.
pub async fn restore(location: &Path, original: &Path) -> io::Result<()> {
	let location = &item_behind(location);
	if tokio::fs::symlink_metadata(original).await.is_ok() {
		return Err(io::Error::new(
			io::ErrorKind::AlreadyExists,
			format!("{} is already there", original.display()),
		));
	}
	if let Some(parent) = original.parent() {
		tokio::fs::create_dir_all(parent).await?;
	}
	if tokio::fs::symlink_metadata(location).await.is_ok() {
		tokio::fs::rename(location, original).await?;
		remove_trash_info(location).await;
		return Ok(());
	}
	let (location, original) = (location.to_path_buf(), original.to_path_buf());
	tokio::task::spawn_blocking(move || restore_os(&location, &original))
		.await
		.map_err(|error| io::Error::other(error.to_string()))?
}

/// Remove a trashed item for good.
pub async fn purge(location: &Path) -> io::Result<()> {
	let location = &item_behind(location);
	match tokio::fs::symlink_metadata(location).await {
		Ok(meta) => {
			if meta.is_dir() {
				tokio::fs::remove_dir_all(location).await?;
			} else {
				tokio::fs::remove_file(location).await?;
			}
			remove_trash_info(location).await;
			Ok(())
		}
		Err(_) => {
			let location = location.to_path_buf();
			tokio::task::spawn_blocking(move || purge_os(&location))
				.await
				.map_err(|error| io::Error::other(error.to_string()))?
		}
	}
}

/// Whether a trash location is a Spacedrive trash directory rather than the
/// platform's.
pub fn is_spacedrive_trash(location: &Path) -> bool {
	location
		.components()
		.collect::<Vec<_>>()
		.windows(2)
		.any(|pair| pair[0].as_os_str() == ".spacedrive" && pair[1].as_os_str() == "trash")
}

/// Remove every Spacedrive trash directory on the mounted volumes, and
/// answer with how many items went.
pub async fn empty_spacedrive_trash(volumes: &VolumeManager) -> io::Result<u64> {
	let mut removed = 0;
	for volume in volumes.get_all_volumes().await {
		let trash = volume.mount_point.join(SPACEDRIVE_TRASH);
		let Ok(mut jobs) = tokio::fs::read_dir(&trash).await else {
			continue;
		};
		while let Ok(Some(job)) = jobs.next_entry().await {
			if let Ok(mut items) = tokio::fs::read_dir(job.path()).await {
				while let Ok(Some(_)) = items.next_entry().await {
					removed += 1;
				}
			}
		}
		tokio::fs::remove_dir_all(&trash).await?;
	}
	Ok(removed)
}

/// Empty the platform's trash.
pub async fn empty_os_trash() -> io::Result<()> {
	tokio::task::spawn_blocking(empty_os)
		.await
		.map_err(|error| io::Error::other(error.to_string()))?
}

/// A rename into the Spacedrive trash directory at the root of the volume
/// holding `path`.
async fn spacedrive_trash(
	path: &Path,
	volumes: Option<&VolumeManager>,
	job: JobId,
) -> io::Result<PathBuf> {
	let volume = match volumes {
		Some(volumes) => volumes.volume_for_path(path).await,
		None => None,
	}
	.ok_or_else(|| io::Error::other("no volume holds the path"))?;
	if volume.is_read_only {
		return Err(io::Error::other("the volume is read only"));
	}
	let directory = volume
		.mount_point
		.join(SPACEDRIVE_TRASH)
		.join(job.to_string());
	tokio::fs::create_dir_all(&directory).await?;
	let name = path
		.file_name()
		.map(|name| name.to_string_lossy().into_owned())
		.unwrap_or_else(|| "item".to_string());
	let mut location = directory.join(&name);
	let mut counter = 1;
	while tokio::fs::symlink_metadata(&location).await.is_ok() {
		location = directory.join(format!("{name}.{counter}"));
		counter += 1;
	}
	tokio::fs::rename(path, &location).await?;
	Ok(location)
}

#[cfg(target_os = "macos")]
fn trash_os(path: &Path) -> io::Result<Option<PathBuf>> {
	use objc2::rc::Retained;
	use objc2_foundation::{NSFileManager, NSString, NSURL};

	let manager = NSFileManager::defaultManager();
	let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
	let mut resulting: Option<Retained<NSURL>> = None;
	manager
		.trashItemAtURL_resultingItemURL_error(&url, Some(&mut resulting))
		.map_err(|error| io::Error::other(error.localizedDescription().to_string()))?;
	Ok(resulting
		.and_then(|url| url.path())
		.map(|path| PathBuf::from(path.to_string())))
}

#[cfg(target_os = "macos")]
fn restore_os(location: &Path, _original: &Path) -> io::Result<()> {
	Err(io::Error::new(
		io::ErrorKind::NotFound,
		format!("{} is no longer in the trash", location.display()),
	))
}

#[cfg(target_os = "macos")]
fn purge_os(location: &Path) -> io::Result<()> {
	Err(io::Error::new(
		io::ErrorKind::NotFound,
		format!("{} is no longer in the trash", location.display()),
	))
}

#[cfg(target_os = "macos")]
fn empty_os() -> io::Result<()> {
	let status = std::process::Command::new("osascript")
		.args(["-e", "tell application \"Finder\" to empty trash"])
		.status()?;
	if status.success() {
		Ok(())
	} else {
		Err(io::Error::other("Finder did not empty the trash"))
	}
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn trash_os(path: &Path) -> io::Result<Option<PathBuf>> {
	trash::delete(path).map_err(|error| io::Error::other(error.to_string()))?;
	let items = trash::os_limited::list().map_err(|error| io::Error::other(error.to_string()))?;
	Ok(items
		.into_iter()
		.filter(|item| item.original_path() == path)
		.max_by_key(|item| item.time_deleted)
		.map(|item| item_location(&item)))
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn trashed_item(location: &Path) -> io::Result<trash::TrashItem> {
	trash::os_limited::list()
		.map_err(|error| io::Error::other(error.to_string()))?
		.into_iter()
		.find(|item| item_location(item) == location)
		.ok_or_else(|| {
			io::Error::new(
				io::ErrorKind::NotFound,
				format!("{} is no longer in the trash", location.display()),
			)
		})
}

/// Where a trashed item sits.
///
/// The `trash` crate's item id on Linux is the `.trashinfo` path, not the
/// item; the freedesktop layout puts the item at `files/<name>` beside
/// `info/<name>.trashinfo`, so the location is derived the way the crate
/// derives it for its own restore.
#[cfg(target_os = "linux")]
fn item_location(item: &trash::TrashItem) -> PathBuf {
	let info = Path::new(&item.id);
	match (info.parent().and_then(Path::parent), info.file_stem()) {
		(Some(trash_dir), Some(name)) => trash_dir.join("files").join(name),
		_ => info.to_path_buf(),
	}
}

#[cfg(target_os = "windows")]
fn item_location(item: &trash::TrashItem) -> PathBuf {
	PathBuf::from(&item.id)
}

/// Remove the `.trashinfo` companion of an item that left `Trash/files` by
/// a rename or a removal, so the trash does not list a ghost.
///
/// The item is already back, or already gone, by the time this runs, so a
/// companion that will not unlink is logged rather than turned into a
/// failure of a restore that landed.
#[cfg(target_os = "linux")]
async fn remove_trash_info(location: &Path) {
	let Some(info) = trash_info_path(location) else {
		return;
	};
	if let Err(error) = tokio::fs::remove_file(&info).await {
		if error.kind() != io::ErrorKind::NotFound {
			tracing::warn!(info = %info.display(), %error, "trashinfo left behind");
		}
	}
}

#[cfg(not(target_os = "linux"))]
async fn remove_trash_info(_location: &Path) {}

/// The item a journaled location names.
///
/// A journal written before the location was the item itself holds the
/// `.trashinfo` path on Linux; restoring that would rename the info file
/// over the original. Such a location is mapped to its `files/<name>`
/// item, so an old journal row restores the bytes.
#[cfg(target_os = "linux")]
fn item_behind(location: &Path) -> PathBuf {
	let is_info = location.extension().is_some_and(|ext| ext == "trashinfo")
		&& location
			.parent()
			.and_then(Path::file_name)
			.is_some_and(|dir| dir == "info");
	match (
		is_info,
		location.parent().and_then(Path::parent),
		location.file_stem(),
	) {
		(true, Some(trash_dir), Some(name)) => trash_dir.join("files").join(name),
		_ => location.to_path_buf(),
	}
}

#[cfg(not(target_os = "linux"))]
fn item_behind(location: &Path) -> PathBuf {
	location.to_path_buf()
}

/// The `info/<name>.trashinfo` companion of a `files/<name>` location, or
/// none when the location is not in a freedesktop trash.
#[cfg(target_os = "linux")]
fn trash_info_path(location: &Path) -> Option<PathBuf> {
	let files = location.parent()?;
	if files.file_name()? != "files" {
		return None;
	}
	let mut name = location.file_name()?.to_os_string();
	name.push(".trashinfo");
	Some(files.parent()?.join("info").join(name))
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn restore_os(location: &Path, _original: &Path) -> io::Result<()> {
	let item = trashed_item(location)?;
	trash::os_limited::restore_all([item]).map_err(|error| io::Error::other(error.to_string()))
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn purge_os(location: &Path) -> io::Result<()> {
	let item = trashed_item(location)?;
	trash::os_limited::purge_all([item]).map_err(|error| io::Error::other(error.to_string()))
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn empty_os() -> io::Result<()> {
	let items = trash::os_limited::list().map_err(|error| io::Error::other(error.to_string()))?;
	trash::os_limited::purge_all(items).map_err(|error| io::Error::other(error.to_string()))
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn trash_os(_path: &Path) -> io::Result<Option<PathBuf>> {
	Err(io::Error::new(
		io::ErrorKind::Unsupported,
		"this platform has no trash",
	))
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn restore_os(_location: &Path, _original: &Path) -> io::Result<()> {
	Err(io::Error::new(
		io::ErrorKind::Unsupported,
		"this platform has no trash",
	))
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn purge_os(_location: &Path) -> io::Result<()> {
	Err(io::Error::new(
		io::ErrorKind::Unsupported,
		"this platform has no trash",
	))
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn empty_os() -> io::Result<()> {
	Err(io::Error::new(
		io::ErrorKind::Unsupported,
		"this platform has no trash",
	))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn a_spacedrive_trash_location_is_told_apart() {
		assert!(is_spacedrive_trash(Path::new(
			"/Volumes/NAS/.spacedrive/trash/abc/file.txt"
		)));
		assert!(!is_spacedrive_trash(Path::new("/Users/me/.Trash/file.txt")));
	}

	/// The trash reports where the item went, and the item comes back from
	/// there with its bytes.
	#[cfg(any(target_os = "macos", target_os = "linux"))]
	#[tokio::test]
	async fn the_trash_reports_where_an_item_went_and_gives_it_back() {
		let dir = tempfile::tempdir().expect("tempdir");
		let file = dir.path().join("gone.txt");
		std::fs::write(&file, b"bytes").expect("file");
		let location = trash(&file, None, JobId::new())
			.await
			.expect("trashed")
			.expect("a location");
		assert!(!file.exists());
		assert!(location.exists(), "{}", location.display());
		restore(&location, &file).await.expect("restored");
		assert_eq!(std::fs::read(&file).expect("back"), b"bytes");
		assert!(!location.exists(), "{} left the trash", location.display());
	}

	/// On Linux the location is the item under `Trash/files`, not its
	/// `.trashinfo`, and restoring it takes the `.trashinfo` with it so the
	/// trash no longer lists the item.
	#[cfg(target_os = "linux")]
	#[tokio::test]
	async fn a_linux_location_is_the_item_and_its_trashinfo_goes_with_it() {
		let dir = tempfile::tempdir().expect("tempdir");
		let file = dir.path().join("gone.txt");
		std::fs::write(&file, b"bytes").expect("file");
		let location = trash(&file, None, JobId::new())
			.await
			.expect("trashed")
			.expect("a location");
		assert_eq!(
			location.parent().and_then(Path::file_name),
			Some(std::ffi::OsStr::new("files")),
			"{}",
			location.display()
		);
		let info = trash_info_path(&location).expect("a trashinfo path");
		assert!(info.exists(), "{}", info.display());
		assert!(trash::os_limited::list()
			.expect("listed")
			.iter()
			.any(|item| item_location(item) == location));

		restore(&location, &file).await.expect("restored");
		assert_eq!(std::fs::read(&file).expect("back"), b"bytes");
		// Another thread trashing a same-named file may take the freed
		// info name at once, so the proof is that the trash no longer
		// lists this item, not that the path is absent.
		assert!(!trash::os_limited::list()
			.expect("listed")
			.iter()
			.any(|item| item.original_path() == file));

		let location = trash(&file, None, JobId::new())
			.await
			.expect("trashed again")
			.expect("a location");
		purge(&location).await.expect("purged");
		assert!(!location.exists());
		assert!(!trash::os_limited::list()
			.expect("listed")
			.iter()
			.any(|item| item.original_path() == file));
	}

	/// A journal row from before the location was the item names the
	/// `.trashinfo` path; restoring it still brings the bytes back rather
	/// than the trashinfo text.
	#[cfg(target_os = "linux")]
	#[tokio::test]
	async fn a_legacy_trashinfo_location_restores_the_item() {
		let dir = tempfile::tempdir().expect("tempdir");
		let file = dir.path().join("old.txt");
		std::fs::write(&file, b"bytes").expect("file");
		let location = trash(&file, None, JobId::new())
			.await
			.expect("trashed")
			.expect("a location");
		let info = trash_info_path(&location).expect("a trashinfo path");
		assert_eq!(item_behind(&info), location);

		restore(&info, &file)
			.await
			.expect("restored from the info path");
		assert_eq!(std::fs::read(&file).expect("back"), b"bytes");
		assert!(!location.exists());
		assert!(!trash::os_limited::list()
			.expect("listed")
			.iter()
			.any(|item| item.original_path() == file));
	}
}
