use clap::Args;
use std::path::PathBuf;

use sd_core::{
	domain::addressing::{SdPath, SdPathBatch},
	infra::job::journal::Attributes,
	ops::{
		files::{
			archive::{ArchiveFormat, FileArchiveInput, FileExtractInput},
			attributes_action::FileSetAttributesInput,
			copy::input::{CopyMethod, FileCopyInput},
			delete::{DeleteTargets, Duplicates, FileDeleteInput, Keep},
			link::{FileLinkInput, LinkKind},
			merge::{FileMergeInput, MergeConflictPolicy},
			organize::{
				FileFlattenInput, FileOrganizeInput, FlattenPolicy, Granularity, OrganizeDateField,
				OrganizeRule,
			},
			rename::{CaseRule, ExtensionCase, FileRenameBatchInput, FileRenameInput, RenameRule},
			trash_view::FileTrashEmptyInput,
			undo::FileUndoInput,
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

	/// Remove from the destination what no source holds, so it ends up
	/// matching the sources: a mirror. The extras go to the trash
	#[arg(long, default_value_t = false)]
	pub remove_extras: bool,

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
			remove_extras: self.remove_extras,
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

#[derive(Args, Debug, Clone)]
pub struct FileDedupeArgs {
	/// The folder whose surplus copies go. Optional with --keep, where it
	/// limits where the other copies are removed from
	pub scope: Option<PathBuf>,

	/// These files stay; every other copy of their content goes
	#[arg(long, value_name = "FILE", num_args = 1.., conflicts_with = "keep_under")]
	pub keep: Vec<PathBuf>,

	/// Remove from the folder what this folder already holds, matched by
	/// content wherever it sits
	#[arg(long, value_name = "DIR", conflicts_with = "keep")]
	pub keep_under: Option<PathBuf>,

	/// Leave contents smaller than this many bytes alone
	#[arg(long, value_name = "BYTES")]
	pub min_size: Option<u64>,

	/// Include hidden files when matching against --keep-under
	#[arg(long, default_value_t = false, requires = "keep_under")]
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

impl FileDedupeArgs {
	pub fn into_input(self) -> anyhow::Result<FileDeleteInput> {
		let scope = self.scope.as_ref().map(local_path).transpose()?;
		let targets = if let Some(under) = &self.keep_under {
			let a = scope
				.ok_or_else(|| anyhow::anyhow!("--keep-under needs the folder to delete from"))?;
			DeleteTargets::Comparison {
				comparison: Comparison {
					a,
					b: local_path(under)?,
					by: CompareBy::Content,
					show: CompareSet::Both,
					include_hidden: self.include_hidden,
				},
			}
		} else if !self.keep.is_empty() {
			DeleteTargets::Duplicates {
				duplicates: Duplicates {
					scope,
					keep: Keep::These {
						paths: self
							.keep
							.iter()
							.map(local_path)
							.collect::<anyhow::Result<_>>()?,
					},
					min_size: self.min_size,
				},
			}
		} else {
			let scope =
				scope.ok_or_else(|| anyhow::anyhow!("name the folder to look for copies in"))?;
			DeleteTargets::Duplicates {
				duplicates: Duplicates {
					scope: Some(scope),
					keep: Keep::First,
					min_size: self.min_size,
				},
			}
		};
		Ok(FileDeleteInput {
			targets,
			permanent: self.permanent,
			recursive: true,
		})
	}
}

#[derive(Args, Debug, Clone)]
pub struct FileRenameArgs {
	/// The files or folders to rename (one or more)
	pub paths: Vec<PathBuf>,

	/// The new name, for one path
	#[arg(long, conflicts_with_all = ["replace", "case", "prefix", "suffix", "sequence", "template"])]
	pub to: Option<String>,

	/// Replace text in the stem: FIND WITH
	#[arg(long, num_args = 2, value_names = ["FIND", "WITH"])]
	pub replace: Option<Vec<String>>,

	/// Treat FIND as a regular expression, with $1 captures in WITH
	#[arg(long, default_value_t = false, requires = "replace")]
	pub regex: bool,

	/// Replace in the whole name rather than the stem
	#[arg(long, default_value_t = false, requires = "replace")]
	pub whole_name: bool,

	/// Change the stem's case
	#[arg(long, value_enum)]
	pub case: Option<CaseArg>,

	/// Lowercase the extension
	#[arg(long, default_value_t = false)]
	pub lower_extension: bool,

	/// Text before the stem
	#[arg(long)]
	pub prefix: Option<String>,

	/// Text after the stem, before the extension
	#[arg(long)]
	pub suffix: Option<String>,

	/// The stem from a pattern holding {n}, such as "IMG_{n:04}", counted over the paths in order
	#[arg(long)]
	pub sequence: Option<String>,

	/// Where the counter starts
	#[arg(long, default_value_t = 1)]
	pub start: u64,

	/// How much the counter grows by
	#[arg(long, default_value_t = 1)]
	pub step: u64,

	/// The whole name from a pattern: {name}, {ext}, {n}, {parent}, {date:%Y-%m-%d}, {captured:%Y-%m-%d}
	#[arg(long)]
	pub template: Option<String>,

	/// Validate and show the plan, then stop
	#[arg(long, default_value_t = false)]
	pub dry_run: bool,

	/// Skip the confirmation prompt
	#[arg(long, short = 'y', default_value_t = false)]
	pub yes: bool,
}

#[derive(clap::ValueEnum, Debug, Clone, Copy)]
pub enum CaseArg {
	Lower,
	Upper,
	Title,
}

/// What a rename command dispatches: one name, or rules over several paths.
pub enum RenameRequest {
	One(FileRenameInput),
	Rules(FileRenameBatchInput),
}

impl FileRenameArgs {
	/// One rename with `--to`, else the rules in a fixed order: replace,
	/// case, affix, sequence, template.
	pub fn into_request(self) -> anyhow::Result<RenameRequest> {
		if self.paths.is_empty() {
			anyhow::bail!("name at least one path");
		}
		if let Some(to) = self.to {
			if self.paths.len() != 1 {
				anyhow::bail!("--to renames one path; give rules to rename several");
			}
			return Ok(RenameRequest::One(FileRenameInput::new(
				local_path(&self.paths[0])?,
				to,
			)));
		}
		let mut rules = Vec::new();
		if let Some(replace) = self.replace {
			rules.push(RenameRule::Replace {
				find: replace[0].clone(),
				with: replace[1].clone(),
				regex: self.regex,
				whole_name: self.whole_name,
			});
		}
		if self.case.is_some() || self.lower_extension {
			rules.push(RenameRule::Case {
				stem: match self.case {
					Some(CaseArg::Lower) => CaseRule::Lower,
					Some(CaseArg::Upper) => CaseRule::Upper,
					Some(CaseArg::Title) => CaseRule::Title,
					None => CaseRule::Keep,
				},
				extension: if self.lower_extension {
					ExtensionCase::Lower
				} else {
					ExtensionCase::Keep
				},
			});
		}
		if self.prefix.is_some() || self.suffix.is_some() {
			rules.push(RenameRule::Affix {
				prefix: self.prefix.unwrap_or_default(),
				suffix: self.suffix.unwrap_or_default(),
			});
		}
		if let Some(pattern) = self.sequence {
			rules.push(RenameRule::Sequence {
				pattern,
				start: self.start,
				step: self.step,
			});
		}
		if let Some(pattern) = self.template {
			rules.push(RenameRule::Template { pattern });
		}
		if rules.is_empty() {
			anyhow::bail!("give --to for one path, or at least one rule");
		}
		Ok(RenameRequest::Rules(FileRenameBatchInput {
			targets: self
				.paths
				.iter()
				.map(local_path)
				.collect::<anyhow::Result<_>>()?,
			rules,
		}))
	}
}

#[derive(Args, Debug, Clone)]
pub struct FileUndoArgs {
	/// The job to undo, by id
	pub job: uuid::Uuid,

	/// Reverse only these effects, by their sequence in the job's journal
	#[arg(long, value_delimiter = ',')]
	pub effects: Option<Vec<i64>>,

	/// Validate and show the plan, then stop
	#[arg(long, default_value_t = false)]
	pub dry_run: bool,

	/// Skip the confirmation prompt
	#[arg(long, short = 'y', default_value_t = false)]
	pub yes: bool,
}

impl FileUndoArgs {
	pub fn into_input(self) -> FileUndoInput {
		FileUndoInput {
			job: self.job,
			effects: self.effects,
		}
	}
}

#[derive(Args, Debug, Clone)]
pub struct FileOrganizeArgs {
	/// The folder whose files move into subfolders
	pub path: PathBuf,

	/// What names the subfolders
	#[arg(long, value_enum)]
	pub by: OrganizeBy,

	/// The date the folders are named by, with --by date
	#[arg(long, value_enum, default_value = "modified")]
	pub field: OrganizeDateField,

	/// How fine the date folders are, with --by date
	#[arg(long, value_enum, default_value = "year-month")]
	pub granularity: Granularity,

	/// Take the files beneath the folder at any depth
	#[arg(long, default_value_t = false)]
	pub recursive: bool,

	/// Validate and show the plan, then stop
	#[arg(long, default_value_t = false)]
	pub dry_run: bool,

	/// Skip the confirmation prompt
	#[arg(long, short = 'y', default_value_t = false)]
	pub yes: bool,
}

#[derive(clap::ValueEnum, Debug, Clone, Copy)]
pub enum OrganizeBy {
	Date,
	Kind,
	Extension,
}

impl FileOrganizeArgs {
	pub fn into_input(self) -> anyhow::Result<FileOrganizeInput> {
		Ok(FileOrganizeInput {
			scope: local_path(&self.path)?,
			rule: match self.by {
				OrganizeBy::Date => OrganizeRule::ByDate {
					field: self.field,
					granularity: self.granularity,
				},
				OrganizeBy::Kind => OrganizeRule::ByKind,
				OrganizeBy::Extension => OrganizeRule::ByExtension,
			},
			recursive: self.recursive,
		})
	}
}

#[derive(Args, Debug, Clone)]
pub struct FileFlattenArgs {
	/// The folder every file beneath it moves up to
	pub path: PathBuf,

	/// What to do with a file whose name is taken at the root
	#[arg(long, value_enum, default_value = "keep-both")]
	pub on_conflict: FlattenPolicy,

	/// Validate and show the plan, then stop
	#[arg(long, default_value_t = false)]
	pub dry_run: bool,

	/// Skip the confirmation prompt
	#[arg(long, short = 'y', default_value_t = false)]
	pub yes: bool,
}

impl FileFlattenArgs {
	pub fn into_input(self) -> anyhow::Result<FileFlattenInput> {
		Ok(FileFlattenInput {
			scope: local_path(&self.path)?,
			on_conflict: self.on_conflict,
		})
	}
}

#[derive(Args, Debug, Clone)]
pub struct FileArchiveArgs {
	/// Files and folders to put in the archive
	#[arg(required = true)]
	pub sources: Vec<PathBuf>,

	/// The archive to write; its name decides the format unless --format says
	#[arg(long, value_name = "ARCHIVE")]
	pub to: PathBuf,

	#[arg(long, value_enum)]
	pub format: Option<ArchiveFormat>,

	/// Trash the sources once the archive is complete
	#[arg(long, default_value_t = false)]
	pub remove_sources: bool,

	/// Validate and show the plan, then stop
	#[arg(long, default_value_t = false)]
	pub dry_run: bool,

	/// Skip the confirmation prompt
	#[arg(long, short = 'y', default_value_t = false)]
	pub yes: bool,
}

impl FileArchiveArgs {
	pub fn into_input(self) -> anyhow::Result<FileArchiveInput> {
		let name = self
			.to
			.file_name()
			.map(|name| name.to_string_lossy().into_owned())
			.unwrap_or_default();
		let format = match (self.format, ArchiveFormat::of_name(&name)) {
			(Some(format), _) => format,
			(None, Some(format)) => format,
			(None, None) => anyhow::bail!("name the archive .zip or .tar.zst, or give --format"),
		};
		Ok(FileArchiveInput {
			sources: self
				.sources
				.iter()
				.map(local_path)
				.collect::<anyhow::Result<_>>()?,
			destination: local_path(&self.to)?,
			format,
			remove_sources: self.remove_sources,
		})
	}
}

#[derive(Args, Debug, Clone)]
pub struct FileExtractArgs {
	/// The archive to extract
	pub archive: PathBuf,

	/// The existing folder to extract into
	#[arg(long, value_name = "DIR")]
	pub to: PathBuf,

	/// What to do with an entry whose file is already there
	#[arg(long, value_enum, default_value = "skip")]
	pub on_conflict: MergeConflictPolicy,

	/// Leading path components to drop from every entry
	#[arg(long, default_value_t = 0)]
	pub strip_components: u32,

	/// Validate and show the plan, then stop
	#[arg(long, default_value_t = false)]
	pub dry_run: bool,

	/// Skip the confirmation prompt
	#[arg(long, short = 'y', default_value_t = false)]
	pub yes: bool,
}

impl FileExtractArgs {
	pub fn into_input(self) -> anyhow::Result<FileExtractInput> {
		Ok(FileExtractInput {
			archive: local_path(&self.archive)?,
			destination: local_path(&self.to)?,
			on_conflict: self.on_conflict,
			strip_components: self.strip_components,
		})
	}
}

#[derive(Args, Debug, Clone)]
pub struct FileAttributesArgs {
	/// The files to change
	#[arg(required = true)]
	pub paths: Vec<PathBuf>,

	/// The permission mode, in octal, such as 644
	#[arg(long)]
	pub mode: Option<String>,

	/// The modification time, as RFC 3339
	#[arg(long)]
	pub modified: Option<String>,

	/// Whether the files are hidden
	#[arg(long)]
	pub hidden: Option<bool>,

	/// Validate and show the plan, then stop
	#[arg(long, default_value_t = false)]
	pub dry_run: bool,

	/// Skip the confirmation prompt
	#[arg(long, short = 'y', default_value_t = false)]
	pub yes: bool,
}

impl FileAttributesArgs {
	pub fn into_input(self) -> anyhow::Result<FileSetAttributesInput> {
		let mode = match &self.mode {
			Some(mode) => Some(
				u32::from_str_radix(mode, 8)
					.map_err(|_| anyhow::anyhow!("--mode takes octal digits, such as 644"))?,
			),
			None => None,
		};
		let modified_ms = match &self.modified {
			Some(when) => Some(
				chrono::DateTime::parse_from_rfc3339(when)
					.map_err(|error| anyhow::anyhow!("--modified takes RFC 3339: {error}"))?
					.timestamp_millis(),
			),
			None => None,
		};
		Ok(FileSetAttributesInput {
			paths: self
				.paths
				.iter()
				.map(local_path)
				.collect::<anyhow::Result<_>>()?,
			attributes: Attributes {
				mode,
				modified_ms,
				hidden: self.hidden,
			},
		})
	}
}

#[derive(Args, Debug, Clone)]
pub struct FileLinkArgs {
	/// Where the link goes
	pub at: PathBuf,

	/// What the link points at
	#[arg(long)]
	pub target: PathBuf,

	/// A hard link rather than a symlink; one volume, files only
	#[arg(long, default_value_t = false)]
	pub hard: bool,

	/// Validate and show the plan, then stop
	#[arg(long, default_value_t = false)]
	pub dry_run: bool,

	/// Skip the confirmation prompt
	#[arg(long, short = 'y', default_value_t = false)]
	pub yes: bool,
}

impl FileLinkArgs {
	pub fn into_input(self) -> anyhow::Result<FileLinkInput> {
		Ok(FileLinkInput {
			at: local_path(&self.at)?,
			target: local_path(&self.target)?,
			kind: if self.hard {
				LinkKind::Hardlink
			} else {
				LinkKind::Symlink
			},
		})
	}
}

#[derive(clap::Subcommand, Debug, Clone)]
pub enum TrashCmd {
	/// What the journals put in the trash, newest first
	List {
		/// Stop after this many items
		#[arg(long)]
		limit: Option<u32>,
	},
	/// Put an item back where it was
	Restore {
		/// The job that trashed it
		job: uuid::Uuid,
		/// The effect's sequence in that job's journal, as `trash list` shows
		sequence: i64,
		/// Skip the confirmation prompt
		#[arg(long, short = 'y', default_value_t = false)]
		yes: bool,
	},
	/// Remove for good what the journals put in the trash and the Spacedrive trash directories
	Empty {
		/// Empty the platform's own trash as well
		#[arg(long, default_value_t = false)]
		os: bool,
		/// Skip the confirmation prompt
		#[arg(long, short = 'y', default_value_t = false)]
		yes: bool,
	},
}

impl TrashCmd {
	pub fn empty_input(os: bool) -> FileTrashEmptyInput {
		FileTrashEmptyInput { os_trash: os }
	}
}
