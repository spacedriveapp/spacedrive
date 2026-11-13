//! A path names one record however it is spelled, and only the spelling on
//! disk names it.
//!
//! On a case-insensitive volume a case-only rename (`Readme.md` to
//! `README.md`) leaves the old spelling answering a stat, so a removal check
//! built on a stat kept the old record beside the new one. These tests rename
//! files and folders by case while the daemon is stopped and while it runs,
//! and expect exactly one record under the new spelling with its identity and
//! tags kept. A case-sensitive volume keeps `Readme.md` and `README.md` as two
//! files.
//!
//! A path reached through another spelling of the same directory (a symlink
//! here, a firmlink such as `/Users` on macOS) finds the record the index holds
//! under the volume's spelling, and is answered in the spelling it was asked
//! in.
//!
//! The case-insensitive volume is the temporary directory on macOS, and an
//! ext4 image formatted with casefold on Linux, which needs passwordless sudo,
//! a loop device and a kernel built with casefold. Where there is none the
//! case-insensitive tests skip with a reason.

mod helpers;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use helpers::TestConfigBuilder;
use sd_core::{
	domain::{addressing::SdPath, File},
	infra::{action::LibraryAction, api::SessionContext, query::LibraryQuery},
	library::Library,
	ops::{
		files::query::{
			directory_listing::{DirectoryListingInput, DirectoryListingQuery, DirectorySortBy},
			FileByPathQuery,
		},
		sources::track::action::{TrackSourceAction, TrackSourceInput},
		tags::{
			apply::{action::ApplyTagsAction, input::ApplyTagsInput, input::TagTargets},
			create::{action::CreateTagAction, input::CreateTagInput},
		},
	},
	Core,
};
use tempfile::TempDir;
use uuid::Uuid;

async fn boot(data_dir: &Path) -> Arc<Core> {
	let mut config = TestConfigBuilder::new(data_dir.to_path_buf())
		.build()
		.expect("config");
	config.services.fs_watcher_enabled = true;
	config.save().expect("save config");
	Arc::new(Core::new(data_dir.to_path_buf()).await.expect("core"))
}

async fn only_library(core: &Core) -> Arc<Library> {
	core.libraries
		.list()
		.await
		.into_iter()
		.next()
		.expect("default library")
}

fn session(library: &Library) -> SessionContext {
	let mut session = SessionContext::device_session(
		sd_core::device::get_current_device_id(),
		sd_core::device::get_current_device_slug(),
	);
	session.current_library_id = Some(library.id());
	session
}

/// Sorted names a listing returns. `File::name` is the stem.
async fn list_dir(core: &Arc<Core>, library: &Arc<Library>, path: PathBuf) -> Vec<String> {
	let listing = DirectoryListingQuery::from_input(DirectoryListingInput {
		path: SdPath::Physical {
			device_slug: sd_core::device::get_current_device_slug(),
			path,
		},
		limit: None,
		include_hidden: Some(false),
		sort_by: DirectorySortBy::Name,
		folders_first: Some(false),
		overlay: None,
	})
	.expect("input")
	.execute(core.context.clone(), session(library))
	.await
	.expect("listing");
	let mut names: Vec<String> = listing.files.iter().map(|file| file.name.clone()).collect();
	names.sort();
	names
}

/// Poll a listing until it reads `expected`, and return what it last read.
async fn listing_settles(
	core: &Arc<Core>,
	library: &Arc<Library>,
	path: PathBuf,
	expected: &[&str],
) -> Vec<String> {
	let mut listed = Vec::new();
	for _ in 0..150 {
		listed = list_dir(core, library, path.clone()).await;
		if listed == expected {
			break;
		}
		tokio::time::sleep(Duration::from_millis(200)).await;
	}
	listed
}

async fn file_info(core: &Arc<Core>, library: &Arc<Library>, path: PathBuf) -> Option<File> {
	FileByPathQuery::new(path)
		.execute(core.context.clone(), session(library))
		.await
		.expect("file info")
}

/// Track a root and wait for its first walk. Returns the root the source
/// holds, which is the volume's spelling of it.
async fn track(core: &Arc<Core>, library: &Arc<Library>, root: &Path) -> PathBuf {
	let output = TrackSourceAction::from_input(TrackSourceInput {
		path: root.to_path_buf(),
		name: None,
		overrides: Default::default(),
	})
	.expect("input")
	.execute(library.clone(), core.context.clone())
	.await
	.expect("track");
	if let Some(walk) = library
		.jobs()
		.get_job(sd_core::infra::job::prelude::JobId(
			output.job_id.expect("tracking dispatched a walk"),
		))
		.await
	{
		walk.wait().await.expect("walk");
	}
	output.root
}

/// Tag the file at `path` and return its record and the tag.
async fn tag(core: &Arc<Core>, library: &Arc<Library>, path: &Path) -> (Uuid, Uuid) {
	let record = file_info(core, library, path.to_path_buf())
		.await
		.unwrap_or_else(|| panic!("{} is indexed", path.display()))
		.id;
	let tag = CreateTagAction::from_input(CreateTagInput {
		path: format!("Spelling/{}", Uuid::new_v4()),
		color: None,
		icon: None,
	})
	.expect("tag input")
	.execute(library.clone(), core.context.clone())
	.await
	.expect("create tag")
	.tag;
	ApplyTagsAction::from_input(ApplyTagsInput {
		targets: TagTargets::File(vec![record]),
		tag_ids: vec![tag.id],
	})
	.expect("apply input")
	.execute(library.clone(), core.context.clone())
	.await
	.expect("apply tag");
	(record, tag.id)
}

/// The record at `path` keeps `record` and `tag`.
async fn kept(core: &Arc<Core>, library: &Arc<Library>, path: &Path, record: Uuid, tag: Uuid) {
	let file = file_info(core, library, path.to_path_buf())
		.await
		.unwrap_or_else(|| panic!("{} is indexed", path.display()));
	assert_eq!(file.id, record, "{} keeps its record", path.display());
	assert!(
		file.tags.iter().any(|applied| applied.id == tag),
		"{} keeps its tag: {:?}",
		path.display(),
		file.tags
	);
}

/// A directory on a case-insensitive volume, unmounted when dropped.
struct Folding {
	root: PathBuf,
	_temp: Option<TempDir>,
	#[cfg(target_os = "linux")]
	_mount: Option<casefold::Mount>,
}

impl Folding {
	fn root(&self) -> &Path {
		&self.root
	}
}

/// Whether `dir` answers a stat for a spelling it does not store.
fn folds_case(dir: &Path) -> bool {
	let probe = dir.join("Probe");
	if std::fs::write(&probe, b"probe").is_err() {
		return false;
	}
	let folds = std::fs::symlink_metadata(dir.join("PROBE")).is_ok();
	std::fs::remove_file(&probe).ok();
	folds
}

/// A case-insensitive directory to work in, or why there is none here.
async fn folding_dir(name: &str) -> Result<Folding, String> {
	#[cfg(target_os = "linux")]
	{
		let mount = casefold::Mount::new(name)?;
		let root = mount.dir.clone();
		if !folds_case(&root) {
			return Err("the casefold directory does not fold case".into());
		}
		Ok(Folding {
			root,
			_temp: None,
			_mount: Some(mount),
		})
	}
	#[cfg(not(target_os = "linux"))]
	{
		let temp = tempfile::Builder::new()
			.prefix(name)
			.tempdir()
			.map_err(|e| e.to_string())?;
		if !folds_case(temp.path()) {
			return Err("the temporary directory is on a case-sensitive volume".into());
		}
		Ok(Folding {
			root: temp.path().to_path_buf(),
			_temp: Some(temp),
		})
	}
}

#[cfg(target_os = "linux")]
mod casefold {
	use std::path::PathBuf;
	use std::process::Command;

	/// An ext4 image formatted with casefold, loop-mounted, with one
	/// directory flagged to fold case and owned by the test's user.
	pub struct Mount {
		pub dir: PathBuf,
		mount_point: PathBuf,
		_holder: tempfile::TempDir,
	}

	fn run(program: &str, args: &[&str]) -> Result<String, String> {
		let output = Command::new(program)
			.args(args)
			.output()
			.map_err(|e| format!("{program}: {e}"))?;
		if !output.status.success() {
			return Err(format!(
				"{program} {}: {}",
				args.join(" "),
				String::from_utf8_lossy(&output.stderr).trim()
			));
		}
		Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
	}

	impl Mount {
		pub fn new(name: &str) -> Result<Self, String> {
			run("sudo", &["-n", "true"]).map_err(|e| format!("no passwordless sudo: {e}"))?;
			let holder = tempfile::Builder::new()
				.prefix(name)
				.tempdir()
				.map_err(|e| e.to_string())?;
			let image = holder.path().join("casefold.img");
			let mount_point = holder.path().join("mnt");
			std::fs::create_dir(&mount_point).map_err(|e| e.to_string())?;
			std::fs::File::create(&image)
				.and_then(|file| file.set_len(64 * 1024 * 1024))
				.map_err(|e| e.to_string())?;
			let image = image.to_str().unwrap();
			run("mkfs.ext4", &["-q", "-F", "-O", "casefold", image])
				.or_else(|_| {
					run(
						"sudo",
						&["-n", "mkfs.ext4", "-q", "-F", "-O", "casefold", image],
					)
				})
				.map_err(|e| format!("mkfs.ext4 -O casefold: {e}"))?;
			run(
				"sudo",
				&[
					"-n",
					"mount",
					"-o",
					"loop",
					image,
					mount_point.to_str().unwrap(),
				],
			)
			.map_err(|e| format!("mounting the casefold image: {e}"))?;
			let mount = Self {
				dir: mount_point.join("fold"),
				mount_point,
				_holder: holder,
			};
			let dir = mount.dir.to_str().unwrap().to_string();
			let owner = format!("{}:{}", run("id", &["-u"])?, run("id", &["-g"])?);
			run("sudo", &["-n", "mkdir", &dir])?;
			run("sudo", &["-n", "chattr", "+F", &dir])
				.map_err(|e| format!("this kernel cannot fold case on ext4: {e}"))?;
			run("sudo", &["-n", "chown", &owner, &dir])?;
			Ok(mount)
		}
	}

	impl Drop for Mount {
		fn drop(&mut self) {
			run(
				"sudo",
				&["-n", "umount", self.mount_point.to_str().unwrap()],
			)
			.ok();
		}
	}
}

/// What a test needs once a source is tracked and walked.
struct Tracked {
	core: Arc<Core>,
	library: Arc<Library>,
	root: PathBuf,
	data_dir: TempDir,
}

async fn tracked(dir: &Path, files: &[&str]) -> Tracked {
	for file in files {
		let path = dir.join(file);
		std::fs::create_dir_all(path.parent().unwrap()).expect("mkdir");
		std::fs::write(&path, file).expect("write");
	}
	let data_dir = tempfile::tempdir().expect("data dir");
	let core = boot(data_dir.path()).await;
	let library = only_library(&core).await;
	let root = track(&core, &library, dir).await;
	Tracked {
		core,
		library,
		root,
		data_dir,
	}
}

/// Stop the daemon cleanly, keeping its data directory for a restart.
async fn stop(tracked: Tracked) -> TempDir {
	let Tracked {
		core,
		library,
		root,
		data_dir,
	} = tracked;
	if let Some(store) = core.context.volume_index().store_for(&root).await {
		store.flush().await.ok();
	}
	drop(library);
	core.shutdown().await.expect("shutdown");
	drop(core);
	tokio::time::sleep(Duration::from_secs(2)).await;
	data_dir
}

async fn restart(data_dir: &TempDir) -> (Arc<Core>, Arc<Library>) {
	let core = boot(data_dir.path()).await;
	let library = only_library(&core).await;
	(core, library)
}

#[tokio::test]
async fn a_case_only_rename_while_stopped_leaves_one_record() {
	let _ = tracing_subscriber::fmt::try_init();
	let folding = match folding_dir("sd-case-stopped").await {
		Ok(folding) => folding,
		Err(reason) => {
			eprintln!("skipping: {reason}");
			return;
		}
	};
	let dir = folding.root();
	let source = tracked(dir, &["Readme.md", "keep/inner.txt", "other.txt"]).await;
	let root = source.root.clone();
	let (readme, readme_tag) = tag(&source.core, &source.library, &root.join("Readme.md")).await;
	let (inner, inner_tag) = tag(&source.core, &source.library, &root.join("keep/inner.txt")).await;
	let data_dir = stop(source).await;

	std::fs::rename(dir.join("Readme.md"), dir.join("README.md")).expect("rename file");
	std::fs::rename(dir.join("keep"), dir.join("Keep")).expect("rename folder");
	assert!(
		std::fs::symlink_metadata(dir.join("Readme.md")).is_ok(),
		"the old spelling still answers a stat, which is the case under test"
	);

	let (core, library) = restart(&data_dir).await;
	let expected = ["Keep", "README", "other"];
	assert_eq!(
		listing_settles(&core, &library, root.clone(), &expected).await,
		expected,
		"one entry per file, under the new spelling"
	);
	assert_eq!(
		list_dir(&core, &library, root.join("Keep")).await,
		["inner"]
	);
	assert!(
		file_info(&core, &library, root.join("Readme.md"))
			.await
			.is_none(),
		"the old spelling has no record"
	);
	assert!(file_info(&core, &library, root.join("keep/inner.txt"))
		.await
		.is_none());
	kept(&core, &library, &root.join("README.md"), readme, readme_tag).await;
	kept(
		&core,
		&library,
		&root.join("Keep/inner.txt"),
		inner,
		inner_tag,
	)
	.await;

	drop(library);
	core.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn a_case_only_rename_under_the_watcher_leaves_one_record() {
	let _ = tracing_subscriber::fmt::try_init();
	let folding = match folding_dir("sd-case-live").await {
		Ok(folding) => folding,
		Err(reason) => {
			eprintln!("skipping: {reason}");
			return;
		}
	};
	let dir = folding.root();
	let source = tracked(dir, &["f01.txt", "keep/inner.txt", "other.txt"]).await;
	let (core, library, root) = (
		source.core.clone(),
		source.library.clone(),
		source.root.clone(),
	);
	let (file, file_tag) = tag(&core, &library, &root.join("f01.txt")).await;
	let (inner, inner_tag) = tag(&core, &library, &root.join("keep/inner.txt")).await;

	// Give the watcher time to settle on the fresh source before renaming.
	tokio::time::sleep(Duration::from_secs(2)).await;
	std::fs::rename(dir.join("f01.txt"), dir.join("F01.txt")).expect("rename file");
	std::fs::rename(dir.join("keep"), dir.join("Keep")).expect("rename folder");

	let expected = ["F01", "Keep", "other"];
	assert_eq!(
		listing_settles(&core, &library, root.clone(), &expected).await,
		expected,
		"one entry per file, under the new spelling"
	);
	// Late events for the old spelling must not bring it back.
	tokio::time::sleep(Duration::from_secs(3)).await;
	assert_eq!(list_dir(&core, &library, root.clone()).await, expected);
	assert_eq!(
		list_dir(&core, &library, root.join("Keep")).await,
		["inner"]
	);
	kept(&core, &library, &root.join("F01.txt"), file, file_tag).await;
	kept(
		&core,
		&library,
		&root.join("Keep/inner.txt"),
		inner,
		inner_tag,
	)
	.await;

	// A restart's reconcile agrees with what the watcher left.
	drop((core, library));
	let data_dir = stop(source).await;
	let (core, library) = restart(&data_dir).await;
	tokio::time::sleep(Duration::from_secs(2)).await;
	assert_eq!(
		listing_settles(&core, &library, root.clone(), &expected).await,
		expected
	);
	kept(&core, &library, &root.join("F01.txt"), file, file_tag).await;

	drop(library);
	core.shutdown().await.expect("shutdown");
}

/// On a case-sensitive volume two spellings are two files, before and after
/// a restart.
#[tokio::test]
async fn two_spellings_stay_two_files_on_a_case_sensitive_volume() {
	let _ = tracing_subscriber::fmt::try_init();
	let temp = tempfile::Builder::new()
		.prefix("sd-case-sensitive")
		.tempdir()
		.expect("temp dir");
	if folds_case(temp.path()) {
		eprintln!("skipping: the temporary directory is on a case-insensitive volume");
		return;
	}
	let dir = temp.path();
	let source = tracked(dir, &["Readme.md", "other.txt"]).await;
	let root = source.root.clone();
	let data_dir = stop(source).await;

	std::fs::write(dir.join("README.md"), "a second file").expect("write");

	let (core, library) = restart(&data_dir).await;
	let expected = ["README", "Readme", "other"];
	assert_eq!(
		listing_settles(&core, &library, root.clone(), &expected).await,
		expected
	);
	let lower = file_info(&core, &library, root.join("Readme.md"))
		.await
		.expect("Readme.md");
	let upper = file_info(&core, &library, root.join("README.md"))
		.await
		.expect("README.md");
	assert_ne!(lower.id, upper.id, "two files, two records");

	std::fs::remove_file(dir.join("README.md")).expect("remove");
	let expected = ["Readme", "other"];
	assert_eq!(
		listing_settles(&core, &library, root.clone(), &expected).await,
		expected,
		"removing one spelling leaves the other"
	);

	drop(library);
	core.shutdown().await.expect("shutdown");
}

/// A path reached through another spelling of the same directory finds the
/// record, and is answered in the spelling it was asked in. A symlink stands
/// in for a macOS firmlink: both reach one directory by two paths.
#[cfg(unix)]
#[tokio::test]
async fn another_spelling_of_a_tracked_folder_finds_the_record() {
	let _ = tracing_subscriber::fmt::try_init();
	let temp = tempfile::Builder::new()
		.prefix("sd-spelling")
		.tempdir()
		.expect("temp dir");
	let real = temp.path().join("real");
	std::fs::create_dir(&real).expect("mkdir");
	let real = real.canonicalize().expect("canonical");
	let alias = temp.path().join("alias");
	std::os::unix::fs::symlink(&real, &alias).expect("symlink");

	let source = tracked(&real, &["a.txt", "sub/b.txt"]).await;
	let (core, library, root) = (
		source.core.clone(),
		source.library.clone(),
		source.root.clone(),
	);

	let held = file_info(&core, &library, root.join("a.txt"))
		.await
		.expect("the held spelling");
	for path in [
		alias.join("a.txt"),
		alias.join("sub").join("..").join("a.txt"),
	] {
		let asked = file_info(&core, &library, path.clone())
			.await
			.unwrap_or_else(|| panic!("{} finds the record", path.display()));
		assert_eq!(asked.id, held.id);
		assert_eq!(
			asked.sd_path.as_local_path(),
			Some(path.as_path()),
			"answered in the spelling it was asked in"
		);
	}
	let nested = file_info(&core, &library, alias.join("sub/b.txt"))
		.await
		.expect("a nested file through the other spelling");
	assert_eq!(
		Some(nested.id),
		file_info(&core, &library, root.join("sub/b.txt"))
			.await
			.map(|file| file.id)
	);
	assert!(file_info(&core, &library, alias.join("missing.txt"))
		.await
		.is_none());

	drop((core, library));
	stop(source).await;
}
