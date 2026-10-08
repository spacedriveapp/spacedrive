//! Where each file goes, read from the index.
//!
//! Both operations are moves inside one folder, so every file keeps its
//! record and the plan is `Move` rows into `CreateDirectory` rows. Organize
//! names a subfolder per file from its date, its content kind, or its
//! extension; flatten names the folder itself. Two files wanting one place
//! are a conflict organize leaves alone and flatten resolves by its policy,
//! numbering the later one or leaving it where it is. Preflight and the job
//! read the same way, so the job moves exactly what the plan showed, and
//! checks each place is still free before it renames.

use std::{
	collections::HashSet,
	path::{Path, PathBuf},
};

use chrono::{DateTime, Local, TimeZone};
use sd_store::FsEntry;

use super::input::{
	FileFlattenInput, FileOrganizeInput, FlattenPolicy, Granularity, OrganizeDateField,
	OrganizeRule,
};
use crate::{
	domain::{content_identity::ContentKind, SdPath},
	infra::query::QueryError,
	ops::{
		files::{plan::StoreRevision, planner::is_junk, rename::resolve::captured_time},
		indexing::VolumeIndex,
		paths::{
			compare::{Folder, Keyed},
			reach::stores_beneath_in,
		},
	},
	volume::VolumeManager,
};

/// The name a file goes under when it has no extension.
pub const NO_EXTENSION: &str = "No extension";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedMove {
	pub from: PathBuf,
	pub to: PathBuf,
	pub size: u64,
}

/// Why a file stays where it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stays {
	/// Another file of the operation, or one already there, holds the place.
	Conflict,
	/// The policy leaves it.
	Policy,
}

#[derive(Debug, Default)]
pub struct Rearrangement {
	pub moves: Vec<PlannedMove>,
	/// Folders that have to exist first, in order.
	pub directories: Vec<PathBuf>,
	/// Files left where they are, with the place they wanted.
	pub left: Vec<(PathBuf, PathBuf, Stays)>,
	/// Files already where the rule puts them.
	pub in_place: u64,
	pub revisions: Vec<StoreRevision>,
}

#[derive(Debug, thiserror::Error)]
pub enum PlanError {
	#[error("{0} is not on this device")]
	Remote(SdPath),
	#[error("{0} is not in a tracked source")]
	Untracked(SdPath),
	#[error("{0}")]
	Read(#[from] QueryError),
}

/// Where organize puts each file.
pub async fn organize(
	volumes: &VolumeManager,
	index: &VolumeIndex,
	input: &FileOrganizeInput,
) -> Result<Rearrangement, PlanError> {
	let (root, files, revisions) = files_beneath(volumes, index, &input.scope).await?;
	let mut out = Rearrangement {
		revisions,
		..Default::default()
	};
	let mut claimed: HashSet<PathBuf> = HashSet::new();
	let mut directories: Vec<PathBuf> = Vec::new();
	for file in files {
		if !input.recursive && !file.key.0.is_empty() {
			continue;
		}
		let folder = match &input.rule {
			OrganizeRule::ByDate { field, granularity } => {
				let when = date_of(volumes, index, &file, *field).await;
				folder_for_date(when, *granularity)
			}
			OrganizeRule::ByKind => folder_for_kind(&file.entry),
			OrganizeRule::ByExtension => file
				.entry
				.extension
				.as_deref()
				.filter(|extension| !extension.is_empty())
				.map(str::to_lowercase)
				.unwrap_or_else(|| NO_EXTENSION.to_string()),
		};
		let directory = root.join(&folder);
		let to = directory.join(&file.entry.name);
		if to == file.path {
			out.in_place += 1;
			continue;
		}
		if !directories.contains(&directory) {
			directories.push(directory);
		}
		place(&mut out, &mut claimed, file, to, None).await;
	}
	for directory in directories {
		if tokio::fs::symlink_metadata(&directory).await.is_err() {
			out.directories.push(directory);
		}
	}
	Ok(out)
}

/// Where flatten puts each file.
pub async fn flatten(
	volumes: &VolumeManager,
	index: &VolumeIndex,
	input: &FileFlattenInput,
) -> Result<Rearrangement, PlanError> {
	let (root, files, revisions) = files_beneath(volumes, index, &input.scope).await?;
	let mut out = Rearrangement {
		revisions,
		..Default::default()
	};
	let mut claimed: HashSet<PathBuf> = files
		.iter()
		.filter(|file| file.key.0.is_empty())
		.map(|file| file.path.clone())
		.collect();
	for file in files {
		if file.key.0.is_empty() {
			out.in_place += 1;
			continue;
		}
		let to = root.join(&file.entry.name);
		place(&mut out, &mut claimed, file, to, Some(input.on_conflict)).await;
	}
	Ok(out)
}

/// Settle one file's place: free, taken by the policy's numbered name, or
/// left with why.
async fn place(
	out: &mut Rearrangement,
	claimed: &mut HashSet<PathBuf>,
	file: Keyed,
	to: PathBuf,
	policy: Option<FlattenPolicy>,
) {
	let size = file.entry.size.unwrap_or(0).max(0) as u64;
	let taken = claimed.contains(&to) || tokio::fs::symlink_metadata(&to).await.is_ok();
	if !taken {
		claimed.insert(to.clone());
		out.moves.push(PlannedMove {
			from: file.path,
			to,
			size,
		});
		return;
	}
	match policy {
		Some(FlattenPolicy::KeepBoth) => {
			let numbered = numbered(&to, claimed).await;
			claimed.insert(numbered.clone());
			out.moves.push(PlannedMove {
				from: file.path,
				to: numbered,
				size,
			});
		}
		Some(FlattenPolicy::Skip) => out.left.push((file.path, to, Stays::Policy)),
		None => out.left.push((file.path, to, Stays::Conflict)),
	}
}

/// A numbered name beside `path` that neither the disk nor the plan holds.
async fn numbered(path: &Path, claimed: &HashSet<PathBuf>) -> PathBuf {
	let parent = path.parent().unwrap_or(Path::new(""));
	let name = path
		.file_name()
		.map(|name| name.to_string_lossy().into_owned())
		.unwrap_or_default();
	let (stem, extension) = match name.rsplit_once('.') {
		Some((stem, extension)) if !stem.is_empty() => (stem.to_string(), format!(".{extension}")),
		_ => (name.clone(), String::new()),
	};
	for counter in 1.. {
		let candidate = parent.join(format!("{stem} ({counter}){extension}"));
		if !claimed.contains(&candidate) && tokio::fs::symlink_metadata(&candidate).await.is_err() {
			return candidate;
		}
	}
	unreachable!("the counter never runs out")
}

/// Every file beneath the scope from the index, junk left out, with the
/// scope as the volume spells it and the revisions read.
async fn files_beneath(
	volumes: &VolumeManager,
	index: &VolumeIndex,
	scope: &SdPath,
) -> Result<(PathBuf, Vec<Keyed>, Vec<StoreRevision>), PlanError> {
	let local = scope
		.as_local_path()
		.ok_or_else(|| PlanError::Remote(scope.clone()))?;
	let root = match volumes.locate_path(local).await {
		Some((_, spelled)) => spelled,
		None => local.to_path_buf(),
	};
	let reaches = stores_beneath_in(volumes, index, scope).await;
	if reaches.is_empty() {
		return Err(PlanError::Untracked(scope.clone()));
	}
	let mut folder = Folder::open(index, reaches, false, None).await?;
	let revisions = folder
		.revisions()
		.await?
		.into_iter()
		.map(|(source, revision)| StoreRevision { source, revision })
		.collect();
	let mut files = Vec::new();
	while let Some(file) = folder.next().await? {
		if is_junk(&file.entry.name) {
			continue;
		}
		files.push(file);
	}
	Ok((root, files, revisions))
}

async fn date_of(
	volumes: &VolumeManager,
	index: &VolumeIndex,
	file: &Keyed,
	field: OrganizeDateField,
) -> DateTime<Local> {
	let modified = file
		.entry
		.mtime_ms
		.and_then(|ms| Local.timestamp_millis_opt(ms).single())
		.unwrap_or_else(Local::now);
	match field {
		OrganizeDateField::Modified => modified,
		OrganizeDateField::Created => file
			.entry
			.created_ms
			.and_then(|ms| Local.timestamp_millis_opt(ms).single())
			.unwrap_or(modified),
		OrganizeDateField::Captured => captured_time(volumes, index, &SdPath::local(&file.path))
			.await
			.unwrap_or(modified),
	}
}

fn folder_for_date(when: DateTime<Local>, granularity: Granularity) -> String {
	match granularity {
		Granularity::Year => when.format("%Y").to_string(),
		Granularity::YearMonth => when.format("%Y-%m").to_string(),
		Granularity::YearMonthDay => when.format("%Y-%m-%d").to_string(),
	}
}

/// The folder a content kind's files go under.
pub fn folder_for_kind(entry: &FsEntry) -> String {
	let kind = entry
		.content_kind
		.and_then(|kind| i32::try_from(kind).ok())
		.map(ContentKind::from_id)
		.filter(|kind| *kind != ContentKind::Unknown)
		.unwrap_or_else(|| {
			crate::filetype::FileTypeRegistry::current()
				.identify_by_extension(Path::new(&entry.name))
		});
	match kind {
		ContentKind::Image => "Images",
		ContentKind::Video => "Videos",
		ContentKind::Audio => "Audio",
		ContentKind::Document => "Documents",
		ContentKind::Archive => "Archives",
		ContentKind::Code => "Code",
		ContentKind::Text => "Text",
		ContentKind::Database => "Databases",
		ContentKind::Book => "Books",
		ContentKind::Font => "Fonts",
		ContentKind::Mesh => "Meshes",
		ContentKind::Config => "Config",
		ContentKind::Encrypted => "Encrypted",
		ContentKind::Key => "Keys",
		ContentKind::Executable => "Executables",
		ContentKind::Binary => "Binaries",
		ContentKind::Spreadsheet => "Spreadsheets",
		ContentKind::Presentation => "Presentations",
		ContentKind::Email => "Email",
		ContentKind::Calendar => "Calendars",
		ContentKind::Contact => "Contacts",
		ContentKind::Web => "Web",
		ContentKind::Shortcut => "Shortcuts",
		ContentKind::Package => "Packages",
		ContentKind::ModelEntry => "Models",
		ContentKind::Memory => "Memory",
		ContentKind::Unknown => "Other",
	}
	.to_string()
}

/// The directories beneath the scope, deepest first, for pruning after a
/// flatten.
pub async fn directories_beneath(root: &Path) -> Vec<PathBuf> {
	let mut found = Vec::new();
	let mut stack = vec![root.to_path_buf()];
	while let Some(directory) = stack.pop() {
		let Ok(mut entries) = tokio::fs::read_dir(&directory).await else {
			continue;
		};
		while let Ok(Some(entry)) = entries.next_entry().await {
			let path = entry.path();
			if tokio::fs::symlink_metadata(&path)
				.await
				.is_ok_and(|meta| meta.is_dir())
			{
				found.push(path.clone());
				stack.push(path);
			}
		}
	}
	found.sort_by(|a, b| b.components().count().cmp(&a.components().count()));
	found
}
