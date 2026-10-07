//! A core with a library and three tracked folders, for tests that read
//! plans from the index and run jobs against the disk.

use std::{
	collections::HashSet,
	path::{Path, PathBuf},
	sync::Arc,
};

use sd_store::{
	file::{FileKind, FileWrite, Ledger, Observation},
	record::ContentIdentity,
};

use crate::{
	infra::{action::preflight::PreviewContext, api::SessionContext},
	library::Library,
};

/// When every fixture file was last modified, unless it says otherwise.
pub(crate) const T: i64 = 1_700_000_000_000;

/// A record to index: its source-relative path, kind, size, modification
/// time, sampled and integrity hashes, and link target.
pub(crate) type Entry<'a> = (
	&'a str,
	FileKind,
	i64,
	i64,
	Option<&'a str>,
	Option<&'a str>,
	Option<&'a str>,
);

pub(crate) struct Fixture {
	_data: tempfile::TempDir,
	_files: tempfile::TempDir,
	pub(crate) core: crate::Core,
	pub(crate) library: Arc<Library>,
	pub(crate) source: PathBuf,
	pub(crate) other: PathBuf,
	pub(crate) destination: PathBuf,
}

impl Fixture {
	pub(crate) async fn new() -> Self {
		let data = tempfile::tempdir().expect("tempdir");
		let files = tempfile::tempdir().expect("tempdir");
		let root = files.path().canonicalize().expect("root");
		let core = crate::Core::new(data.path().to_path_buf())
			.await
			.expect("core");
		let library = core
			.libraries
			.create_library("Fixture", None, core.context.clone())
			.await
			.expect("library");

		// Sources are registered as their volume spells them, which is how a
		// preflight reaches them; on macOS that is the data volume's mount rather
		// than the firmlink the tempdir is made under.
		let mut folders = Vec::new();
		for name in ["src", "other", "dst"] {
			let folder = root.join(name);
			std::fs::create_dir_all(&folder).expect("folder");
			let folder = core
				.context
				.volume_manager
				.locate_path(&folder)
				.await
				.map_or(folder, |(_, spelled)| spelled);
			core.context
				.volume_index()
				.register_source_in(Some(library.id()), &folder, None)
				.await
				.expect("registered");
			folders.push(folder);
		}
		let destination = folders.pop().expect("dst");
		let other = folders.pop().expect("other");
		let source = folders.pop().expect("src");
		Self {
			_data: data,
			_files: files,
			core,
			library,
			source,
			other,
			destination,
		}
	}

	pub(crate) fn session(&self) -> SessionContext {
		let device_id = self
			.core
			.context
			.device_manager
			.device_id()
			.expect("device");
		SessionContext::device_session(device_id, "test".to_string())
			.with_library(self.library.id())
	}

	pub(crate) fn preview(&self) -> PreviewContext {
		PreviewContext::new(&self.core.context, self.library.clone(), self.session())
	}

	/// Write records into a folder's store: the directories above each
	/// entry, then the entry, with its content identity where it has one.
	pub(crate) async fn index(&self, root: &Path, entries: &[Entry<'_>]) {
		let store = self
			.core
			.context
			.volume_index()
			.store_for(root)
			.await
			.expect("store");
		let db = store.db();
		db.begin_sync().await.expect("epoch");
		let mut ledger = Ledger::load(db.pool()).await.expect("ledger");

		let mut writes = Vec::new();
		let mut directories = HashSet::new();
		for (path, kind, size, mtime, _, _, target) in entries {
			for (end, _) in path.match_indices('/') {
				let directory = &path[..end];
				if directories.insert(directory.to_string()) {
					writes.push(write(
						&mut ledger,
						directory,
						FileKind::Directory,
						0,
						T,
						None,
					));
				}
			}
			if *kind == FileKind::Directory {
				directories.insert(path.to_string());
			}
			writes.push(write(&mut ledger, path, *kind, *size, *mtime, *target));
		}
		db.apply_files(&writes, &[], &[], None)
			.await
			.expect("apply");

		for (path, _, size, _, sampled, integrity, _) in entries {
			let Some(sampled) = sampled else {
				continue;
			};
			let record = sd_store::read::entry_by_path(db.pool(), path)
				.await
				.expect("lookup")
				.expect("written")
				.uuid;
			db.set_content_identity(
				record,
				&ContentIdentity {
					sampled_hash: Some(sampled.to_string()),
					integrity_hash: integrity.map(str::to_string),
					size: Some(*size),
					..Default::default()
				},
			)
			.await
			.expect("identity");
		}
	}

	/// Put the same entries on disk beneath `root`, so a preflight check that stats
	/// the filesystem and a job that walks it agree with the index.
	pub(crate) fn materialize(&self, root: &Path, entries: &[Entry<'_>]) {
		for (path, kind, size, _, _, _, target) in entries {
			let at = root.join(path);
			match kind {
				FileKind::Directory => std::fs::create_dir_all(&at).expect("dir"),
				FileKind::Symlink => {
					#[cfg(unix)]
					std::os::unix::fs::symlink(target.unwrap_or("."), &at).expect("link");
				}
				_ => {
					if let Some(parent) = at.parent() {
						std::fs::create_dir_all(parent).expect("parent");
					}
					std::fs::write(&at, vec![b'x'; (*size).max(0) as usize]).expect("file");
				}
			}
		}
	}
}

fn write(
	ledger: &mut Ledger,
	path: &str,
	kind: FileKind,
	size: i64,
	mtime: i64,
	link_target: Option<&str>,
) -> FileWrite {
	let name = path.rsplit('/').next().unwrap_or(path).to_string();
	let observation = Observation {
		external_id: path.to_string(),
		kind,
		is_hidden: name.starts_with('.'),
		extension: name.rsplit_once('.').map(|(_, e)| e.to_string()),
		name,
		size,
		mtime,
		created: None,
		accessed: None,
		inode: None,
		mode: Some(0o644),
		uid: None,
		gid: None,
		link_target: link_target.map(String::from),
		identity: None,
	};
	let resolution = ledger.resolve(&observation);
	let parent_uuid = path
		.rsplit_once('/')
		.and_then(|(parent, _)| ledger.uuid_of(parent));
	FileWrite {
		resolution,
		parent_uuid,
		observation,
	}
}
