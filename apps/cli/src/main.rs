use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use sd_core::client::CoreClient;

#[derive(Debug, Clone, ValueEnum)]
enum OutputFormat {
	Human,
	Json,
}

#[derive(Parser, Debug)]
#[command(name = "spacedrive", about = "Spacedrive v2 CLI (daemon client)")]
struct Cli {
	/// Path to spacedrive data directory
	#[arg(long)]
	data_dir: Option<std::path::PathBuf>,

	/// Output format
	#[arg(long, value_enum, default_value = "human")]
	format: OutputFormat,

	#[command(subcommand)]
	command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
	/// Core info
	Status,
	/// Libraries operations
	#[command(subcommand)]
	Library(LibraryCommands),
	/// File operations
	#[command(subcommand)]
	File(FileCommands),
	/// Indexing operations
	#[command(subcommand)]
	Index(IndexCommands),
}

#[derive(Subcommand, Debug)]
enum LibraryCommands {
	/// List libraries
	List,
}

#[derive(Subcommand, Debug)]
enum FileCommands {
	/// Copy files
	Copy(FileCopyArgs),
	/// Delete files
	Delete(FileDeleteArgs),
	/// Validate files
	Validate(FileValidateArgs),
	/// Detect duplicate files
	Dedupe(FileDedupeArgs),
}

#[derive(Parser, Debug, Clone)]
struct FileCopyArgs {
	/// Source files or directories to copy (one or more)
	pub sources: Vec<std::path::PathBuf>,

	/// Destination path
	#[arg(long)]
	pub destination: std::path::PathBuf,

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
}

impl FileCopyArgs {
	fn to_input(&self) -> sd_core::ops::files::copy::input::FileCopyInput {
		use sd_core::ops::files::copy::input::{CopyMethod, FileCopyInput};
		let mut input = FileCopyInput::new(self.sources.clone(), self.destination.clone())
			.with_overwrite(self.overwrite)
			.with_verification(self.verify_checksum)
			.with_timestamp_preservation(self.preserve_timestamps)
			.with_move(self.move_files)
			.with_copy_method(CopyMethod::Auto);
		input
	}
}

#[derive(Parser, Debug, Clone)]
struct FileDeleteArgs {
	/// Files or directories to delete (one or more)
	pub targets: Vec<std::path::PathBuf>,

	/// Permanently delete instead of moving to trash
	#[arg(long, default_value_t = false)]
	pub permanent: bool,

	/// Delete directories recursively
	#[arg(long, default_value_t = true)]
	pub recursive: bool,
}

impl FileDeleteArgs {
	fn to_input(&self) -> sd_core::ops::files::delete::input::FileDeleteInput {
		use sd_core::domain::addressing::{SdPath, SdPathBatch};
		use sd_core::ops::files::delete::input::FileDeleteInput;
		let paths = self
			.targets
			.iter()
			.cloned()
			.map(SdPath::local)
			.collect::<Vec<_>>();
		FileDeleteInput {
			targets: SdPathBatch::new(paths),
			permanent: self.permanent,
			recursive: self.recursive,
		}
	}
}

#[derive(Parser, Debug, Clone)]
struct FileValidateArgs {
	/// Paths to validate (one or more)
	pub paths: Vec<std::path::PathBuf>,

	/// Verify checksums during validation
	#[arg(long, default_value_t = false)]
	pub verify_checksums: bool,

	/// Perform deep scan
	#[arg(long, default_value_t = false)]
	pub deep_scan: bool,
}

impl FileValidateArgs {
	fn to_input(&self) -> sd_core::ops::files::validation::input::FileValidationInput {
		use sd_core::ops::files::validation::input::FileValidationInput;
		FileValidationInput {
			paths: self.paths.clone(),
			verify_checksums: self.verify_checksums,
			deep_scan: self.deep_scan,
		}
	}
}

#[derive(Debug, Clone, ValueEnum)]
enum DedupeAlgorithmArg {
	ContentHash,
	SizeOnly,
	NameAndSize,
	DeepScan,
}

impl DedupeAlgorithmArg {
	fn as_str(&self) -> &'static str {
		match self {
			Self::ContentHash => "content_hash",
			Self::SizeOnly => "size_only",
			Self::NameAndSize => "name_and_size",
			Self::DeepScan => "deep_scan",
		}
	}
}

#[derive(Parser, Debug, Clone)]
struct FileDedupeArgs {
	/// Paths to scan for duplicates (one or more)
	pub paths: Vec<std::path::PathBuf>,

	/// Detection algorithm
	#[arg(long, value_enum, default_value = "content-hash")]
	pub algorithm: DedupeAlgorithmArg,

	/// Similarity threshold (0.0 - 1.0)
	#[arg(long, default_value_t = 1.0)]
	pub threshold: f64,
}

impl FileDedupeArgs {
	fn to_input(&self) -> sd_core::ops::files::duplicate_detection::input::DuplicateDetectionInput {
		use sd_core::ops::files::duplicate_detection::input::DuplicateDetectionInput;
		DuplicateDetectionInput {
			paths: self.paths.clone(),
			algorithm: self.algorithm.as_str().to_string(),
			threshold: self.threshold,
		}
	}
}

#[derive(Subcommand, Debug)]
enum IndexCommands {
	/// Start indexing for one or more paths
	Start(IndexStartArgs),
}

#[derive(Debug, Clone, ValueEnum)]
enum IndexModeArg { Shallow, Content, Deep }

#[derive(Debug, Clone, ValueEnum)]
enum IndexScopeArg { Current, Recursive }

impl From<IndexModeArg> for sd_core::ops::indexing::job::IndexMode {
	fn from(m: IndexModeArg) -> Self {
		use sd_core::ops::indexing::job::IndexMode as M;
		match m { IndexModeArg::Shallow => M::Shallow, IndexModeArg::Content => M::Content, IndexModeArg::Deep => M::Deep }
	}
}

impl From<IndexScopeArg> for sd_core::ops::indexing::job::IndexScope {
	fn from(s: IndexScopeArg) -> Self {
		use sd_core::ops::indexing::job::IndexScope as S;
		match s { IndexScopeArg::Current => S::Current, IndexScopeArg::Recursive => S::Recursive }
	}
}

#[derive(Parser, Debug, Clone)]
struct IndexStartArgs {
	/// Paths to index (one or more)
	pub paths: Vec<std::path::PathBuf>,

	/// Library ID to run indexing in (defaults to the only library if just one exists)
	#[arg(long)]
	pub library: Option<uuid::Uuid>,

	/// Indexing mode
	#[arg(long, value_enum, default_value = "content")]
	pub mode: IndexModeArg,

	/// Indexing scope
	#[arg(long, value_enum, default_value = "recursive")]
	pub scope: IndexScopeArg,

	/// Include hidden files
	#[arg(long, default_value_t = false)]
	pub include_hidden: bool,

	/// Persist results to the database instead of in-memory
	#[arg(long, default_value_t = false)]
	pub persistent: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
	let cli = Cli::parse();
	let data_dir = cli.data_dir.unwrap_or(sd_core::config::default_data_dir()?);
	let socket = data_dir.join("daemon/daemon.sock");
	let core = CoreClient::new(socket);

	match cli.command {
		Commands::Status => {
			let status: sd_core::ops::core::status::output::CoreStatus = core
				.query(&sd_core::ops::core::status::query::CoreStatusQuery)
				.await?;
			match cli.format {
				OutputFormat::Human => println!(
					"Spacedrive Core {} (libraries: {})",
					status.version, status.library_count
				),
				OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&status)?),
			}
		}
		Commands::Library(LibraryCommands::List) => {
			let libs: Vec<sd_core::ops::libraries::list::output::LibraryInfo> = core
				.query(&sd_core::ops::libraries::list::query::ListLibrariesQuery::basic())
				.await?;
			match cli.format {
				OutputFormat::Human => {
					if libs.is_empty() {
						println!("No libraries found");
					}
					for l in libs {
						println!("- {} {}", l.id, l.path.display());
					}
				}
				OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&libs)?),
			}
		}
		Commands::File(FileCommands::Copy(args)) => {
			let input = args.to_input();
			// Basic validation via core input
			if let Err(errors) = input.validate() {
				anyhow::bail!(errors.join("; "));
			}
			core.action(&input).await?;
			println!("Copy request submitted");
		}
		Commands::File(FileCommands::Delete(args)) => {
			let input = args.to_input();
			if let Err(errors) = input.validate() {
				anyhow::bail!(errors.join("; "));
			}
			core.action(&input).await?;
			println!("Delete request submitted");
		}
		Commands::File(FileCommands::Validate(args)) => {
			let input = args.to_input();
			core.action(&input).await?;
			println!("Validation request submitted");
		}
		Commands::File(FileCommands::Dedupe(args)) => {
			let input = args.to_input();
			core.action(&input).await?;
			println!("Duplicate detection request submitted");
		}
		Commands::Index(IndexCommands::Start(args)) => {
			use sd_core::ops::indexing::input::IndexInput;
			use sd_core::ops::indexing::job::{IndexMode, IndexPersistence, IndexScope};

			let library_id = if let Some(id) = args.library {
				id
			} else {
				// If only one library exists, use it; otherwise require --library
				let libs: Vec<sd_core::ops::libraries::list::output::LibraryInfo> = core
					.query(&sd_core::ops::libraries::list::query::ListLibrariesQuery::basic())
					.await?;
				match libs.len() {
					0 => anyhow::bail!("No libraries found; specify --library after creating one"),
					1 => libs[0].id,
					_ => anyhow::bail!("Multiple libraries found; please specify --library <UUID>"),
				}
			};

			let persistence = if args.persistent {
				IndexPersistence::Persistent
			} else {
				IndexPersistence::Ephemeral
			};

			let input = IndexInput::new(library_id, args.paths.clone())
				.with_mode(IndexMode::from(args.mode.clone()))
				.with_scope(IndexScope::from(args.scope.clone()))
				.with_include_hidden(args.include_hidden)
				.with_persistence(persistence);

			// Validate input
			if let Err(errors) = input.validate() {
				anyhow::bail!(errors.join("; "));
			}

			core.action(&input).await?;
			println!("Indexing request submitted");
		}
	}

	Ok(())
}
