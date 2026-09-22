use clap::Args;
use std::path::PathBuf;

use sd_core::{
	domain::addressing::{SdPath, SdPathBatch},
	ops::{
		files::copy::input::{CopyMethod, FileCopyInput},
		paths::compare::{CompareBy, CompareCursor, CompareSet, PathCompareInput},
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
}

impl From<FileCopyArgs> for FileCopyInput {
	fn from(args: FileCopyArgs) -> Self {
		let sources = args
			.sources
			.iter()
			.map(|p| SdPath::local(p.clone()))
			.collect::<Vec<_>>();
		let destination = SdPath::local(args.destination);
		Self {
			sources: SdPathBatch { paths: sources },
			destination,
			overwrite: args.overwrite,
			verify_checksum: args.verify_checksum,
			preserve_timestamps: args.preserve_timestamps,
			move_files: args.move_files,
			copy_method: args.method,
			on_conflict: None,
		}
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
	/// The folder to compare
	pub left: PathBuf,

	/// The folder to compare it against
	pub right: PathBuf,

	/// Match files by where they sit in their folder, or by their bytes
	/// wherever they sit
	#[arg(long, value_enum, default_value = "location")]
	pub by: CompareByArg,

	/// Which files to list
	#[arg(long, value_enum, default_value = "only-left")]
	pub show: CompareSetArg,

	/// Include hidden files
	#[arg(long, default_value_t = false)]
	pub include_hidden: bool,

	/// Continue after this path, relative to the listed folder, as a previous
	/// page ended
	#[arg(long)]
	pub after: Option<String>,

	/// Files per page, at most 5000
	#[arg(long, default_value_t = 100)]
	pub limit: u32,
}

#[derive(clap::ValueEnum, Debug, Clone, Copy)]
pub enum CompareByArg {
	Location,
	Content,
}

#[derive(clap::ValueEnum, Debug, Clone, Copy)]
pub enum CompareSetArg {
	OnlyLeft,
	OnlyRight,
	Changed,
	Same,
}

impl From<CompareByArg> for CompareBy {
	fn from(by: CompareByArg) -> Self {
		match by {
			CompareByArg::Location => Self::Location,
			CompareByArg::Content => Self::Content,
		}
	}
}

impl From<CompareSetArg> for CompareSet {
	fn from(set: CompareSetArg) -> Self {
		match set {
			CompareSetArg::OnlyLeft => Self::OnlyLeft,
			CompareSetArg::OnlyRight => Self::OnlyRight,
			CompareSetArg::Changed => Self::Changed,
			CompareSetArg::Same => Self::Same,
		}
	}
}

impl FileCompareArgs {
	/// Both folders as this device spells them, so a relative path or a
	/// symlink means what it does in the shell. "local" names whichever device
	/// answers, which is the one the folders were resolved on.
	pub fn into_input(self) -> anyhow::Result<PathCompareInput> {
		let folder = |path: &PathBuf| {
			path.canonicalize()
				.map(|path| SdPath::Physical {
					device_slug: "local".into(),
					path,
				})
				.map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))
		};
		Ok(PathCompareInput {
			left: folder(&self.left)?,
			right: folder(&self.right)?,
			by: self.by.into(),
			show: self.show.into(),
			include_hidden: self.include_hidden,
			after: self.after.map(|path| {
				let (directory, name) = path.rsplit_once('/').unwrap_or(("", &path));
				CompareCursor {
					directory: directory.to_string(),
					name: name.to_string(),
				}
			}),
			limit: self.limit,
		})
	}
}
