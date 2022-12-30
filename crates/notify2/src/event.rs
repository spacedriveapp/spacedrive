use std::path::PathBuf;

use fsevent_stream::stream::StreamFlags;

/// Type of the target of the event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
	Directory,
	File,
	Symlink,
	Hardlink,
}

impl TryFrom<&StreamFlags> for Type {
	type Error = ();

	fn try_from(f: &StreamFlags) -> Result<Self, Self::Error> {
		Ok(match *f {
			f if f.contains(StreamFlags::IS_FILE) => Self::File,
			f if f.contains(StreamFlags::IS_DIR) => Self::Directory,
			f if f.contains(StreamFlags::IS_SYMLINK) => Self::Symlink,
			f if f.contains(StreamFlags::IS_HARDLINK) => Self::Hardlink,
			f if f.contains(StreamFlags::IS_LAST_HARDLINK) => Self::Hardlink,
			_ => return Err(()),
		})
	}
}

/// Filesystem event that is emitted by the watcher.
/// The goal of this library is that all of the events here act the same on all platforms.
/// If that's not the case open an issue on GitHub!
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
	Create(Type, PathBuf),
	Modify(Type, PathBuf),
	Move {
		ty: Type,
		from: PathBuf,
		to: PathBuf,
	},
	Delete(Type, PathBuf),
	Mount(PathBuf),
	Unmount(PathBuf),

	// TODO: The behavior of this event is probs not going to be cross-platform without extra work so do that.
	// macOS will subscribe/unsubscribe as the root is created and deleted. I don't think other platforms do that???
	RootChange(PathBuf),
}
