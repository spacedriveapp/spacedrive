use clap::Args;
use std::path::PathBuf;

use sd_core::{
	domain::addressing::{SdPath, SdPathBatch},
	ops::{
		files::copy::input::{CopyMethod, FileCopyInput},
		paths::compare::{CompareBy, CompareSet, PathCompareInput, MAX_PAGE},
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
	/// Both folders as this device spells them, so a relative path or a
	/// symlink means what it does in the shell. "local" names whichever device
	/// answers, which is the one the folders were resolved on. The input asks
	/// for the first page; walking the rest sets each page's limit and cursor.
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
			a: folder(&self.a)?,
			b: folder(&self.b)?,
			by: self.by.into(),
			show: self.show.into(),
			include_hidden: self.include_hidden,
			after: None,
			limit: MAX_PAGE,
		})
	}
}
