use clap::Args;
use std::path::PathBuf;

use sd_core::{
	domain::addressing::{SdPath, SdPathBatch},
	ops::{
		files::{
			copy::input::{CopyMethod, FileCopyInput},
			delete::{DeleteTargets, FileDeleteInput},
			merge::{FileMergeInput, MergeConflictPolicy},
		},
		paths::compare::{CompareBy, CompareSet, Comparison, PathCompareInput, MAX_PAGE},
	},
};

#[derive(Args, Debug, Clone)]
pub struct FileCopyArgs {
	/// Source files or directories to copy (one or more)
	pub sources: Vec<PathBuf>,

	/// Destination path
	#[arg(long)]
	pub destination: PathBuf,

	/// Overwrite existing files
	#[arg(long, default_value_t = false)]
	pub overwrite: bool,

	/// Verify checksums during copy
	#[arg(long, default_value_t = false)]
	pub verify_checksum: bool,

	/// Preserve file timestamps
	#[arg(long, default_value_t = true)]
	pub preserve_timestamps: bool,

	/// Delete source files after copy (move)
	#[arg(long, default_value_t = false)]
	pub move_files: bool,

	/// Copy method to use
	#[arg(long, default_value_t = CopyMethod::Auto)]
	pub method: CopyMethod,

	/// Validate and show the plan, then stop
	#[arg(long, default_value_t = false)]
	pub dry_run: bool,

	/// Skip the confirmation prompt
	#[arg(long, short = 'y', default_value_t = false)]
	pub yes: bool,
}

impl FileCopyArgs {
	pub fn into_input(self) -> anyhow::Result<FileCopyInput> {
		Ok(FileCopyInput {
			sources: SdPathBatch {
				paths: self
					.sources
					.iter()
					.map(local_path)
					.collect::<anyhow::Result<_>>()?,
			},
			destination: local_path(&self.destination)?,
			overwrite: self.overwrite,
			verify_checksum: self.verify_checksum,
			preserve_timestamps: self.preserve_timestamps,
			move_files: self.move_files,
			copy_method: self.method,
			on_conflict: None,
		})
	}
}

#[derive(Args, Debug, Clone)]
pub struct FileInfoArgs {
	/// File path to get information about
	pub path: PathBuf,
}

#[derive(Args, Debug, Clone)]
pub struct FileListArgs {
	/// Directory path to list contents of
	pub path: PathBuf,

	/// Maximum number of items to return
	#[arg(long)]
	pub limit: Option<u32>,

	/// Include hidden files and directories
	#[arg(long, default_value_t = false)]
	pub include_hidden: bool,

	/// Sort order for the results (name, modified, size, type)
	#[arg(long, default_value = "name")]
	pub sort_by: String,
}

#[derive(Args, Debug, Clone)]
pub struct FileCompareArgs {
	/// The folder to compare, called A in the output
	pub a: PathBuf,

	/// The folder to compare it against, called B
	pub b: PathBuf,

	/// How files match across the two folders
	#[arg(long, value_enum, default_value = "path")]
	pub by: CompareByArg,

	/// Which files to list
	#[arg(long, value_enum, default_value = "only-a")]
	pub show: CompareSetArg,

	/// Include hidden files
	#[arg(long, default_value_t = false)]
	pub include_hidden: bool,

	/// Stop after this many files; every file by default
	#[arg(long, value_parser = clap::value_parser!(u32).range(1..))]
	pub limit: Option<u32>,
}

#[derive(clap::ValueEnum, Debug, Clone, Copy)]
pub enum CompareByArg {
	/// Files match when they sit at the same path in each folder
	Path,
	/// Files match when they hold the same bytes, wherever they sit
	Content,
}

#[derive(clap::ValueEnum, Debug, Clone, Copy)]
pub enum CompareSetArg {
	/// Files in A and not in B
	OnlyA,
	/// Files in B and not in A
	OnlyB,
	/// Files in both folders
	Both,
	/// Files at the same path in both with different bytes, by path only
	Different,
}

impl From<CompareByArg> for CompareBy {
	fn from(by: CompareByArg) -> Self {
		match by {
			CompareByArg::Path => Self::Path,
			CompareByArg::Content => Self::Content,
		}
	}
}

impl From<CompareSetArg> for CompareSet {
	fn from(set: CompareSetArg) -> Self {
		match set {
			CompareSetArg::OnlyA => Self::OnlyA,
			CompareSetArg::OnlyB => Self::OnlyB,
			CompareSetArg::Both => Self::Both,
			CompareSetArg::Different => Self::Different,
		}
	}
}

impl FileCompareArgs {
	/// The input asks for the first page; walking the rest sets each page's
	/// limit and cursor.
	pub fn into_input(self) -> anyhow::Result<PathCompareInput> {
		Ok(PathCompareInput {
			comparison: Comparison {
				a: local_path(&self.a)?,
				b: local_path(&self.b)?,
				by: self.by.into(),
				show: self.show.into(),
				include_hidden: self.include_hidden,
			},
			after: None,
			limit: MAX_PAGE,
		})
	}
}

#[derive(Args, Debug, Clone)]
pub struct FileDeleteArgs {
	/// Files to delete; with --against, the one folder, A, to delete from
	#[arg(required = true)]
	pub paths: Vec<PathBuf>,

	/// Delete from A the files in one set of its comparison with this
	/// folder, B, as `file compare A B` lists them
	#[arg(long, value_name = "B")]
	pub against: Option<PathBuf>,

	/// How files match across the two folders
	#[arg(long, value_enum, default_value = "path", requires = "against")]
	pub by: CompareByArg,

	/// Which set to delete; both is what B already holds
	#[arg(long, value_enum, requires = "against")]
	pub show: Option<CompareSetArg>,

	/// Include hidden files in the comparison
	#[arg(long, default_value_t = false, requires = "against")]
	pub include_hidden: bool,

	/// Delete permanently instead of moving to the trash
	#[arg(long, default_value_t = false)]
	pub permanent: bool,

	/// Validate and show the plan, then stop
	#[arg(long, default_value_t = false)]
	pub dry_run: bool,

	/// Skip the confirmation prompt
	#[arg(long, short = 'y', default_value_t = false)]
	pub yes: bool,
}

impl FileDeleteArgs {
	pub fn into_input(self) -> anyhow::Result<FileDeleteInput> {
		let targets = match self.against {
			Some(b) => {
				let [a] = self.paths.as_slice() else {
					anyhow::bail!("--against compares one folder; name the folder to delete from");
				};
				let Some(show) = self.show else {
					anyhow::bail!(
						"--against needs --show to say which set to delete: both, only-a or different"
					);
				};
				DeleteTargets::Comparison {
					comparison: Comparison {
						a: local_path(a)?,
						b: local_path(&b)?,
						by: self.by.into(),
						show: show.into(),
						include_hidden: self.include_hidden,
					},
				}
			}
			None => DeleteTargets::Paths {
				paths: self
					.paths
					.iter()
					.map(local_path)
					.collect::<anyhow::Result<_>>()?,
			},
		};
		Ok(FileDeleteInput {
			targets,
			permanent: self.permanent,
			recursive: true,
		})
	}
}

#[derive(Args, Debug, Clone)]
pub struct FileMergeArgs {
	/// Folders to merge, in order
	#[arg(required = true)]
	pub sources: Vec<PathBuf>,

	/// The existing folder to merge into
	#[arg(long, value_name = "DIR")]
	pub into: PathBuf,

	/// What to do with a file at the same path whose bytes differ
	#[arg(long, value_enum, default_value = "skip")]
	pub on_conflict: MergeConflictPolicy,

	/// Remove each source leaf once it is merged, and prune emptied folders;
	/// what the merge does not settle stays
	#[arg(long, default_value_t = false)]
	pub consume: bool,

	/// Validate and show the plan, then stop
	#[arg(long, default_value_t = false)]
	pub dry_run: bool,

	/// Skip the confirmation prompt
	#[arg(long, short = 'y', default_value_t = false)]
	pub yes: bool,
}

impl FileMergeArgs {
	pub fn into_input(self) -> anyhow::Result<FileMergeInput> {
		Ok(FileMergeInput {
			sources: SdPathBatch {
				paths: self
					.sources
					.iter()
					.map(local_path)
					.collect::<anyhow::Result<_>>()?,
			},
			destination: local_path(&self.into)?,
			on_conflict: self.on_conflict,
			consume_sources: self.consume,
		})
	}
}

/// A path as this device spells it, so a relative path or a symlink means
/// what it does in the shell. "local" names whichever device answers, which
/// is the one the path was resolved on.
fn local_path(path: &PathBuf) -> anyhow::Result<SdPath> {
	path.canonicalize()
		.map(|path| SdPath::Physical {
			device_slug: "local".into(),
			path,
		})
		.map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))
}
