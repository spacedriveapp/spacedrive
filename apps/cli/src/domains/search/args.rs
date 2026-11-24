use chrono::{DateTime, Utc};
use clap::Args;
use std::path::PathBuf;
use uuid::Uuid;

use sd_core::domain::addressing::SdPath;
use sd_core::ops::indexing::text_extract::TextKind;
use sd_core::ops::search::content::ContentSearchInput;
use sd_core::ops::search::grep::{GrepInput, GrepOptions};

use sd_core::domain::ContentKind;
use sd_core::ops::search::input::{
	DateField, DateRangeFilter, FileSearchInput, PaginationOptions, SearchFilters, SearchMode,
	SearchScope, SizeRangeFilter, SortDirection, SortField, SortOptions, TagFilter,
};

#[derive(Args, Debug)]
pub struct FileSearchArgs {
	/// Search query
	pub query: String,

	/// Search mode
	#[arg(long, value_enum, default_value = "normal")]
	pub mode: SearchModeArg,

	/// SD path to narrow search to a specific directory
	#[arg(long)]
	pub sd_path: Option<String>,

	/// File type filter (can be specified multiple times)
	#[arg(long)]
	pub file_type: Option<Vec<String>>,

	/// Tag filter (can be specified multiple times)
	#[arg(long)]
	pub tags: Option<Vec<Uuid>>,

	/// Exclude tags (can be specified multiple times)
	#[arg(long)]
	pub exclude_tags: Option<Vec<Uuid>>,

	/// Date field for filtering
	#[arg(long, value_enum, default_value = "modified")]
	pub date_field: DateFieldArg,

	/// Start date for filtering (ISO format)
	#[arg(long)]
	pub date_start: Option<DateTime<Utc>>,

	/// End date for filtering (ISO format)
	#[arg(long)]
	pub date_end: Option<DateTime<Utc>>,

	/// Minimum file size in bytes
	#[arg(long)]
	pub min_size: Option<u64>,

	/// Maximum file size in bytes
	#[arg(long)]
	pub max_size: Option<u64>,

	/// Content type filter
	#[arg(long, value_enum)]
	pub content_type: Option<Vec<ContentTypeArg>>,

	/// Sort field
	#[arg(long, value_enum, default_value = "relevance")]
	pub sort_field: SortFieldArg,

	/// Sort direction
	#[arg(long, value_enum, default_value = "desc")]
	pub sort_direction: SortDirectionArg,

	/// Limit number of results
	#[arg(long, default_value = "50")]
	pub limit: u32,

	/// Offset for pagination
	#[arg(long, default_value = "0")]
	pub offset: u32,

	/// Include hidden files
	#[arg(long)]
	pub include_hidden: bool,

	/// Include archived files
	#[arg(long)]
	pub include_archived: bool,

	/// Only photos whose EXIF places them (true) or does not (false)
	#[arg(long)]
	pub has_location: Option<bool>,
}

#[derive(clap::ValueEnum, Debug, Clone)]
pub enum SearchModeArg {
	Fast,
	Normal,
	Full,
}

#[derive(clap::ValueEnum, Debug, Clone)]
pub enum DateFieldArg {
	Created,
	Modified,
	Accessed,
	/// When the photo was taken, from its EXIF
	Captured,
}

#[derive(clap::ValueEnum, Debug, Clone)]
pub enum ContentTypeArg {
	Unknown,
	Image,
	Video,
	Audio,
	Document,
	Archive,
	Code,
	Text,
	Database,
	Book,
	Font,
	Mesh,
	Config,
	Encrypted,
	Key,
	Executable,
	Binary,
}

#[derive(clap::ValueEnum, Debug, Clone)]
pub enum SortFieldArg {
	Relevance,
	Name,
	Size,
	Modified,
	Created,
	/// When the photo was taken, from its EXIF
	Captured,
}

#[derive(clap::ValueEnum, Debug, Clone)]
pub enum SortDirectionArg {
	Asc,
	Desc,
}

impl From<FileSearchArgs> for FileSearchInput {
	fn from(args: FileSearchArgs) -> Self {
		let mode = match args.mode {
			SearchModeArg::Fast => SearchMode::Fast,
			SearchModeArg::Normal => SearchMode::Normal,
			SearchModeArg::Full => SearchMode::Full,
		};

		let scope = if let Some(sd_path_str) = args.sd_path {
			// Parse SD path from string
			match sd_core::domain::addressing::SdPath::from_uri(&sd_path_str) {
				Ok(sd_path) => SearchScope::Path { path: sd_path },
				Err(_) => {
					eprintln!(
						"Warning: Invalid SD path '{}', falling back to library search",
						sd_path_str
					);
					SearchScope::Library
				}
			}
		} else {
			SearchScope::Library
		};

		let filters = SearchFilters {
			file_types: args.file_type,
			tags: if args.tags.is_some() || args.exclude_tags.is_some() {
				Some(TagFilter {
					include: args.tags.unwrap_or_default(),
					exclude: args.exclude_tags.unwrap_or_default(),
				})
			} else {
				None
			},
			date_range: if args.date_start.is_some() || args.date_end.is_some() {
				Some(DateRangeFilter {
					field: match args.date_field {
						DateFieldArg::Created => DateField::CreatedAt,
						DateFieldArg::Modified => DateField::ModifiedAt,
						DateFieldArg::Accessed => DateField::AccessedAt,
						DateFieldArg::Captured => DateField::CapturedAt,
					},
					start: args.date_start,
					end: args.date_end,
				})
			} else {
				None
			},
			size_range: if args.min_size.is_some() || args.max_size.is_some() {
				Some(SizeRangeFilter {
					min: args.min_size,
					max: args.max_size,
				})
			} else {
				None
			},
			content_types: args.content_type.map(|types| {
				types
					.into_iter()
					.map(|ct| match ct {
						ContentTypeArg::Unknown => ContentKind::Unknown,
						ContentTypeArg::Image => ContentKind::Image,
						ContentTypeArg::Video => ContentKind::Video,
						ContentTypeArg::Audio => ContentKind::Audio,
						ContentTypeArg::Document => ContentKind::Document,
						ContentTypeArg::Archive => ContentKind::Archive,
						ContentTypeArg::Code => ContentKind::Code,
						ContentTypeArg::Text => ContentKind::Text,
						ContentTypeArg::Database => ContentKind::Database,
						ContentTypeArg::Book => ContentKind::Book,
						ContentTypeArg::Font => ContentKind::Font,
						ContentTypeArg::Mesh => ContentKind::Mesh,
						ContentTypeArg::Config => ContentKind::Config,
						ContentTypeArg::Encrypted => ContentKind::Encrypted,
						ContentTypeArg::Key => ContentKind::Key,
						ContentTypeArg::Executable => ContentKind::Executable,
						ContentTypeArg::Binary => ContentKind::Binary,
					})
					.collect()
			}),
			include_hidden: Some(args.include_hidden),
			include_archived: Some(args.include_archived),
			has_location: args.has_location,
			at_risk: None,
			on_volumes: None,
			not_on_volumes: None,
			min_volume_count: None,
			max_volume_count: None,
		};

		let sort = SortOptions {
			field: match args.sort_field {
				SortFieldArg::Relevance => SortField::Relevance,
				SortFieldArg::Name => SortField::Name,
				SortFieldArg::Size => SortField::Size,
				SortFieldArg::Modified => SortField::ModifiedAt,
				SortFieldArg::Created => SortField::CreatedAt,
				SortFieldArg::Captured => SortField::CapturedAt,
			},
			direction: match args.sort_direction {
				SortDirectionArg::Asc => SortDirection::Asc,
				SortDirectionArg::Desc => SortDirection::Desc,
			},
		};

		let pagination = PaginationOptions {
			limit: args.limit,
			offset: args.offset,
		};

		Self {
			query: args.query,
			scope,
			mode,
			filters,
			sort,
			pagination,
		}
	}
}

#[derive(Args, Debug)]
pub struct GrepArgs {
	/// Regular expression to search for, or plain text with --fixed-strings
	pub pattern: String,

	/// File or directory to search, under a tracked source; defaults to the
	/// current directory
	pub path: Option<PathBuf>,

	/// Treat the pattern as plain text
	#[arg(short = 'F', long)]
	pub fixed_strings: bool,

	/// Match without regard to case
	#[arg(short = 'i', long)]
	pub ignore_case: bool,

	/// Only search files matching this glob (repeatable)
	#[arg(short = 'g', long = "glob")]
	pub include: Vec<String>,

	/// Skip files and directories matching this glob (repeatable)
	#[arg(long)]
	pub exclude: Vec<String>,

	/// Search files that .gitignore and .ignore files exclude
	#[arg(long)]
	pub no_ignore: bool,

	/// Search hidden files and directories
	#[arg(long)]
	pub hidden: bool,

	/// Lines of context around each match
	#[arg(short = 'C', long, default_value = "0")]
	pub context: u32,

	/// Matches per page
	#[arg(long)]
	pub max_results: Option<u32>,

	/// Continue from the cursor a previous page printed
	#[arg(long)]
	pub cursor: Option<String>,

	/// Skip files larger than this many bytes (default 1 MiB)
	#[arg(long)]
	pub max_file_bytes: Option<u64>,
}

impl GrepArgs {
	pub fn into_input(self, path: PathBuf) -> GrepInput {
		GrepInput {
			// The daemon that answers resolves "local" to itself, which keeps
			// this right when the request is routed to another device.
			scope: SdPath::Physical {
				device_slug: "local".to_string(),
				path,
			},
			options: GrepOptions {
				pattern: self.pattern,
				literal: self.fixed_strings,
				case_insensitive: self.ignore_case,
				include: self.include,
				exclude: self.exclude,
				respect_gitignore: !self.no_ignore,
				include_hidden: self.hidden,
				context_lines: self.context,
				max_results: self.max_results,
				cursor: self.cursor,
				max_file_bytes: self.max_file_bytes,
			},
		}
	}
}

#[derive(Args, Debug)]
pub struct ContentSearchArgs {
	/// Words that must all appear; "quoted text" is a phrase, word* a prefix
	pub query: String,

	/// File or directory to search, under a tracked source; defaults to the
	/// current directory
	pub path: Option<PathBuf>,

	/// Only files of this kind (repeatable): text, markdown, csv, json,
	/// code, pdf, docx, transcript, scene, caption
	#[arg(long = "kind")]
	pub kinds: Vec<String>,

	/// Hits per page
	#[arg(long)]
	pub max_results: Option<u32>,

	/// Continue from the cursor a previous page printed
	#[arg(long)]
	pub cursor: Option<String>,
}

impl ContentSearchArgs {
	pub fn into_input(self, path: PathBuf) -> anyhow::Result<ContentSearchInput> {
		let kinds = self
			.kinds
			.iter()
			.map(|kind| {
				TextKind::parse(kind).ok_or_else(|| anyhow::anyhow!("unknown kind '{kind}'"))
			})
			.collect::<anyhow::Result<_>>()?;
		Ok(ContentSearchInput {
			scope: SdPath::Physical {
				device_slug: "local".to_string(),
				path,
			},
			query: self.query,
			kinds,
			max_results: self.max_results,
			cursor: self.cursor,
		})
	}
}

#[derive(Args, Debug)]
pub struct ReindexContentArgs {
	/// A tracked source, or any path inside it; defaults to the current
	/// directory
	pub path: Option<PathBuf>,
}
