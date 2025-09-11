use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use sd_core::client::CoreClient;
use uuid::Uuid;

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

	/// Daemon instance name
	#[arg(long)]
	instance: Option<String>,

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
	/// Location operations
	#[command(subcommand)]
	Location(LocationCommands),
	/// Networking and pairing
	#[command(subcommand)]
	Network(NetworkCommands),
	/// Job commands
	#[command(subcommand)]
	Job(JobCommands),
}

#[derive(Subcommand, Debug)]
enum LibraryCommands {
	/// List libraries
	List,
	/// Create a library
	Create { name: String, #[arg(long)] path: Option<std::path::PathBuf> },
	/// Rename a library
	Rename { library_id: Uuid, new_name: String },
	/// Delete a library
	Delete { library_id: Uuid },
	/// Export a library
	Export { library_id: Uuid, #[arg(long)] dest: Option<std::path::PathBuf> },
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

#[derive(Subcommand, Debug)]
enum NetworkCommands {
	/// Show networking status
	Status,
	/// List devices
	Devices(NetworkDevicesArgs),
	/// Start networking
	Start,
	/// Stop networking
	Stop,
	/// Pairing commands
	#[command(subcommand)]
	Pair(PairCommands),
	/// Revoke a paired device
	Revoke { device_id: Uuid },
	/// Send files via Spacedrop
	Spacedrop(SpacedropArgs),
}

#[derive(Parser, Debug, Clone)]
struct NetworkDevicesArgs {
	/// Only show paired devices
	#[arg(long, default_value_t = false)]
	paired_only: bool,
	/// Only show connected devices
	#[arg(long, default_value_t = false)]
	connected_only: bool,
}

#[derive(Subcommand, Debug)]
enum PairCommands {
	/// Generate a pairing code (initiator)
	Generate { #[arg(long, default_value_t = false)] auto_accept: bool },
	/// Join using a pairing code (joiner)
	Join { code: String },
	/// Show pairing sessions
	Status,
	/// Cancel a pairing session
	Cancel { session_id: Uuid },
}

#[derive(Subcommand, Debug)]
enum JobCommands {
	/// List jobs
	List { #[arg(long)] status: Option<String> },
	/// Job info
	Info { job_id: Uuid },
}

#[derive(Parser, Debug, Clone)]
struct SpacedropArgs {
	/// Target device ID
	pub device_id: Uuid,
	/// Files or directories to share
	pub paths: Vec<String>,
	/// Sender name for display
	#[arg(long)]
	pub sender: Option<String>,
}

#[derive(Parser, Debug, Clone)]
struct FileCopyArgs {
	/// Source addresses to copy (SdPath URIs or local paths)
	pub sources: Vec<String>,

	/// Destination address (SdPath URI or local path)
	#[arg(long)]
	pub destination: String,

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
		use sd_core::domain::addressing::{SdPath, SdPathBatch};
		use sd_core::ops::files::copy::input::{CopyMethod, FileCopyInput};
		let src_paths = self
			.sources
			.iter()
			.map(|s| SdPath::from_uri(s).unwrap_or_else(|_| SdPath::local(s)))
			.collect::<Vec<_>>();
		let dest_path = SdPath::from_uri(&self.destination)
			.unwrap_or_else(|_| SdPath::local(&self.destination));
		FileCopyInput {
			sources: SdPathBatch::new(src_paths),
			destination: dest_path,
			overwrite: self.overwrite,
			verify_checksum: self.verify_checksum,
			preserve_timestamps: self.preserve_timestamps,
			move_files: self.move_files,
			copy_method: CopyMethod::Auto,
		}
	}
}

#[derive(Parser, Debug, Clone)]
struct FileDeleteArgs {
	/// Addresses to delete (SdPath URIs or local paths)
	pub targets: Vec<String>,

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
			.map(|s| SdPath::from_uri(s).unwrap_or_else(|_| SdPath::local(s)))
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
	/// Addresses to validate (SdPath URIs or local paths)
	pub paths: Vec<String>,

	/// Verify checksums during validation
	#[arg(long, default_value_t = false)]
	pub verify_checksums: bool,

	/// Perform deep scan
	#[arg(long, default_value_t = false)]
	pub deep_scan: bool,
}

impl FileValidateArgs {
	fn to_input(&self) -> sd_core::ops::files::validation::input::FileValidationInput {
		use sd_core::domain::addressing::SdPath;
		use sd_core::ops::files::validation::input::FileValidationInput;
		let mut local_paths: Vec<std::path::PathBuf> = Vec::new();
		for s in &self.paths {
			let sd = SdPath::from_uri(s).unwrap_or_else(|_| SdPath::local(s));
			if let Some(p) = sd.as_local_path() {
				local_paths.push(p.to_path_buf());
			} else {
				anyhow::bail!(format!("Non-local address not supported for validation: {}", s));
			}
		}
		FileValidationInput {
			paths: local_paths,
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
	/// Addresses to scan for duplicates (SdPath URIs or local paths)
	pub paths: Vec<String>,

	/// Detection algorithm
	#[arg(long, value_enum, default_value = "content-hash")]
	pub algorithm: DedupeAlgorithmArg,

	/// Similarity threshold (0.0 - 1.0)
	#[arg(long, default_value_t = 1.0)]
	pub threshold: f64,
}

impl FileDedupeArgs {
	fn to_input(&self) -> sd_core::ops::files::duplicate_detection::input::DuplicateDetectionInput {
		use sd_core::domain::addressing::SdPath;
		use sd_core::ops::files::duplicate_detection::input::DuplicateDetectionInput;
		let mut local_paths: Vec<std::path::PathBuf> = Vec::new();
		for s in &self.paths {
			let sd = SdPath::from_uri(s).unwrap_or_else(|_| SdPath::local(s));
			if let Some(p) = sd.as_local_path() {
				local_paths.push(p.to_path_buf());
			} else {
				anyhow::bail!(format!("Non-local address not supported for duplicate detection: {}", s));
			}
		}
		DuplicateDetectionInput {
			paths: local_paths,
			algorithm: self.algorithm.as_str().to_string(),
			threshold: self.threshold,
		}
	}
}

#[derive(Subcommand, Debug)]
enum IndexCommands {
	/// Start indexing for one or more paths
	Start(IndexStartArgs),
	/// Quick scan of a path (ephemeral)
	QuickScan(QuickScanArgs),
	/// Browse a path without adding as location
	Browse(BrowseArgs),
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
	/// Addresses to index (SdPath URIs or local paths)
	pub paths: Vec<String>,

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

#[derive(Parser, Debug, Clone)]
struct QuickScanArgs {
	pub path: String,
	#[arg(long, value_enum, default_value = "current")]
	pub scope: IndexScopeArg,
}

#[derive(Parser, Debug, Clone)]
struct BrowseArgs {
	pub path: String,
	#[arg(long, value_enum, default_value = "current")]
	pub scope: IndexScopeArg,
	#[arg(long, default_value_t = false)]
	pub content: bool,
}

#[derive(Subcommand, Debug)]
enum LocationCommands {
	Add { path: std::path::PathBuf, #[arg(long)] name: Option<String>, #[arg(long, value_enum, default_value = "content")] mode: IndexModeArg },
	List,
	Remove { location_id: Uuid },
	Rescan { location_id: Uuid, #[arg(long, default_value_t = false)] force: bool },
}

#[tokio::main]
async fn main() -> Result<()> {
	let cli = Cli::parse();
	let data_dir = cli.data_dir.unwrap_or(sd_core::config::default_data_dir()?);
	let socket = if let Some(inst) = cli.instance {
		data_dir.join("daemon").join(format!("daemon-{}.sock", inst))
	} else {
		data_dir.join("daemon/daemon.sock")
	};
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
		Commands::Library(LibraryCommands::Create { name, path }) => {
			use sd_core::ops::libraries::create::input::LibraryCreateInput;
			let mut input = LibraryCreateInput::new(name);
			if let Some(p) = path { input = input.with_path(p); }
			if let Err(errors) = input.validate() { anyhow::bail!(errors.join("; ")); }
			let out: sd_core::ops::libraries::create::output::LibraryCreateOutput = core.action(&input).await?;
			println!("Created library {} at {}", out.id, out.path.display());
		}
		Commands::Library(LibraryCommands::Rename { library_id, new_name }) => {
			use sd_core::ops::libraries::rename::LibraryRenameInput;
			let out: sd_core::ops::libraries::rename::output::LibraryRenameOutput = core.action(&LibraryRenameInput { library_id, new_name }).await?;
			println!("Renamed library {}: {} -> {}", out.library_id, out.old_name, out.new_name);
		}
		Commands::Library(LibraryCommands::Delete { library_id }) => {
			use sd_core::ops::libraries::delete::input::LibraryDeleteInput;
			let _out: sd_core::ops::libraries::delete::output::LibraryDeleteOutput = core.action(&LibraryDeleteInput { library_id }).await?;
			println!("Deleted library {}", library_id);
		}
		Commands::Library(LibraryCommands::Export { library_id, dest }) => {
			use sd_core::ops::libraries::export::input::LibraryExportInput;
			let out: sd_core::ops::libraries::export::output::LibraryExportOutput = core.action(&LibraryExportInput { library_id, destination: dest }).await?;
			println!("Exported library {} to {}", library_id, out.destination.display());
		}
		Commands::File(FileCommands::Copy(args)) => {
			let input = args.to_input();
			// Basic validation via core input
			if let Err(errors) = input.validate() {
				anyhow::bail!(errors.join("; "));
			}
			let _bytes = core.action(&input).await?;
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

			// Convert addresses to local paths for current IndexInput contract
			let mut local_paths: Vec<std::path::PathBuf> = Vec::new();
			for s in &args.paths {
				let sd = sd_core::domain::addressing::SdPath::from_uri(s)
					.unwrap_or_else(|_| sd_core::domain::addressing::SdPath::local(s));
				if let Some(p) = sd.as_local_path() {
					local_paths.push(p.to_path_buf());
				} else {
					anyhow::bail!(format!("Non-local address not supported for indexing yet: {}", s));
				}
			}

			let input = IndexInput::new(library_id, local_paths)
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
		Commands::Index(IndexCommands::QuickScan(args)) => {
			// Placeholder until dedicated input exists; reuse Start with ephemeral current dir
			use sd_core::ops::indexing::input::IndexInput;
			use sd_core::ops::indexing::job::{IndexMode, IndexPersistence, IndexScope};
			let libs: Vec<sd_core::ops::libraries::list::output::LibraryInfo> = core
				.query(&sd_core::ops::libraries::list::query::ListLibrariesQuery::basic())
				.await?;
			let library_id = if libs.len() == 1 { libs[0].id } else { anyhow::bail!("Specify --library for quick-scan when multiple libraries exist") };
			let sd = sd_core::domain::addressing::SdPath::from_uri(&args.path).unwrap_or_else(|_| sd_core::domain::addressing::SdPath::local(&args.path));
			let p = sd.as_local_path().ok_or_else(|| anyhow::anyhow!("Non-local path not supported yet"))?;
			let input = IndexInput::new(library_id, vec![p.to_path_buf()])
				.with_mode(IndexMode::Shallow)
				.with_scope(IndexScope::from(args.scope.clone()))
				.with_persistence(IndexPersistence::Ephemeral);
			core.action(&input).await?;
			println!("Quick scan request submitted");
		}
		Commands::Index(IndexCommands::Browse(args)) => {
			use sd_core::ops::indexing::input::IndexInput;
			use sd_core::ops::indexing::job::{IndexMode, IndexPersistence, IndexScope};
			let libs: Vec<sd_core::ops::libraries::list::output::LibraryInfo> = core
				.query(&sd_core::ops::libraries::list::query::ListLibrariesQuery::basic())
				.await?;
			let library_id = if libs.len() == 1 { libs[0].id } else { anyhow::bail!("Specify --library for browse when multiple libraries exist") };
			let sd = sd_core::domain::addressing::SdPath::from_uri(&args.path).unwrap_or_else(|_| sd_core::domain::addressing::SdPath::local(&args.path));
			let p = sd.as_local_path().ok_or_else(|| anyhow::anyhow!("Non-local path not supported yet"))?;
			let input = IndexInput::new(library_id, vec![p.to_path_buf()])
				.with_mode(if args.content { IndexMode::Content } else { IndexMode::Shallow })
				.with_scope(IndexScope::from(args.scope.clone()))
				.with_persistence(IndexPersistence::Ephemeral);
			core.action(&input).await?;
			println!("Browse request submitted");
		}
		Commands::Location(LocationCommands::Add { path, name, mode }) => {
			let out: sd_core::ops::locations::add::output::LocationAddOutput = core.action(&sd_core::ops::locations::add::action::LocationAddInput { path, name, mode: sd_core::ops::indexing::job::IndexMode::from(mode) }).await?;
			println!("Added location {} -> {}", out.id, out.path.display());
		}
		Commands::Location(LocationCommands::List) => {
			// If only one library exists, use it; else require --library later (omitted for brevity)
			let libs: Vec<sd_core::ops::libraries::list::output::LibraryInfo> = core
				.query(&sd_core::ops::libraries::list::query::ListLibrariesQuery::basic())
				.await?;
			let library_id = if libs.len() == 1 { libs[0].id } else { anyhow::bail!("Specify --library to list locations when multiple libraries exist") };
			let out: sd_core::ops::locations::list::output::LocationsListOutput = core.query(&sd_core::ops::locations::list::query::LocationsListQuery { library_id }).await?;
			for loc in out.locations { println!("- {} {}", loc.id, loc.path.display()); }
		}
		Commands::Location(LocationCommands::Remove { location_id }) => {
			let _out: sd_core::ops::locations::remove::output::LocationRemoveOutput = core.action(&sd_core::ops::locations::remove::action::LocationRemoveInput { location_id }).await?;
			println!("Removed location {}", location_id);
		}
		Commands::Location(LocationCommands::Rescan { location_id, force: _ }) => {
			let _out: sd_core::ops::locations::rescan::output::LocationRescanOutput = core.action(&sd_core::ops::locations::rescan::action::LocationRescanInput { location_id }).await?;
			println!("Rescan requested for {}", location_id);
		}
		Commands::Network(cmd) => {
			use sd_core::ops::network::*;
			match cmd {
				NetworkCommands::Status => {
					let status: NetworkStatus = core
						.query(&NetworkStatusQuery)
						.await?;
					match cli.format {
						OutputFormat::Human => {
							println!("Networking: {}", if status.running { "running" } else { "stopped" });
							if let Some(id) = status.node_id { println!("Node ID: {}", id); }
							if !status.addresses.is_empty() {
								println!("Addresses:");
								for a in status.addresses { println!("  {}", a); }
							}
							println!("Paired: {} | Connected: {}", status.paired_devices, status.connected_devices);
						}
						OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&status)?),
					}
				}
				NetworkCommands::Devices(args) => {
					let q = if args.connected_only { ListDevicesQuery::connected() } else if args.paired_only { ListDevicesQuery::paired() } else { ListDevicesQuery::all() };
					let devices: Vec<DeviceInfoLite> = core.query(&q).await?;
					match cli.format {
						OutputFormat::Human => {
							if devices.is_empty() { println!("No devices found"); }
							for d in devices { println!("- {} {} ({} | {} | {} | last seen {})", d.id, d.name, d.os_version, d.app_version, if d.is_connected { "connected" } else { "offline" }, d.last_seen); }
						}
						OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&devices)?),
					}
				}
				NetworkCommands::Start => {
					let out: NetworkStartOutput = core.action(&NetworkStartInput {}).await?;
					println!("Networking {}", if out.started { "started" } else { "already running" });
				}
				NetworkCommands::Stop => {
					let out: NetworkStopOutput = core.action(&NetworkStopInput {}).await?;
					println!("Networking {}", if out.stopped { "stopped" } else { "not running" });
				}
				NetworkCommands::Pair(pc) => match pc {
					PairCommands::Generate { auto_accept } => {
						let out: PairGenerateOutput = core.action(&PairGenerateInput { auto_accept }).await?;
						match cli.format {
							OutputFormat::Human => {
								println!("Pairing code: {}", out.code);
								println!("Session: {}", out.session_id);
								println!("Expires at: {}", out.expires_at);
							}
							OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&out)?),
						}
					}
					PairCommands::Join { code } => {
						let out: PairJoinOutput = core.action(&PairJoinInput { code }).await?;
						match cli.format {
							OutputFormat::Human => println!("Paired with {} ({})", out.device_name, out.paired_device_id),
							OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&out)?),
						}
					}
					PairCommands::Status => {
						let out: PairStatusOutput = core.query(&PairStatusQuery).await?;
						match cli.format {
							OutputFormat::Human => {
								if out.sessions.is_empty() { println!("No pairing sessions"); }
								for s in out.sessions { println!("- {} {:?} remote={:?}", s.id, s.state, s.remote_device_id); }
							}
							OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&out)?),
						}
					}
					PairCommands::Cancel { session_id } => {
						let out: PairCancelOutput = core.action(&PairCancelInput { session_id }).await?;
						println!("Cancelled: {}", out.cancelled);
					}
				},
				NetworkCommands::Revoke { device_id } => {
					let out: DeviceRevokeOutput = core.action(&DeviceRevokeInput { device_id }).await?;
					println!("Revoked: {}", out.revoked);
				}
				NetworkCommands::Spacedrop(args) => {
					use sd_core::domain::addressing::SdPath;
					let paths = args.paths.iter().map(|s| SdPath::from_uri(s).unwrap_or_else(|_| SdPath::local(s))).collect::<Vec<_>>();
					let out: SpacedropSendOutput = core.action(&SpacedropSendInput { device_id: args.device_id, paths, sender: args.sender }).await?;
					match cli.format {
						OutputFormat::Human => {
							if let Some(j) = out.job_id { println!("Transfer job: {}", j); }
							if let Some(sid) = out.session_id { println!("Spacedrop session: {}", sid); }
						}
						OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&out)?),
					}
				}
			}
		}
		Commands::Job(cmd) => {
			match cmd {
				JobCommands::List { status } => {
					let libs: Vec<sd_core::ops::libraries::list::output::LibraryInfo> = core
						.query(&sd_core::ops::libraries::list::query::ListLibrariesQuery::basic())
						.await?;
					if libs.is_empty() { println!("No libraries found"); }
					for lib in libs {
						let status_parsed = status.as_deref().and_then(|s| s.parse::<sd_core::infra::job::types::JobStatus>().ok());
						let out: sd_core::ops::jobs::list::output::JobListOutput = core.query(&sd_core::ops::jobs::list::query::JobListQuery { library_id: lib.id, status: status_parsed }).await?;
						for j in out.jobs { println!("- {} {} {} {:?}", j.id, j.name, (j.progress * 100.0) as u32, j.status); }
					}
				}
				JobCommands::Info { job_id } => {
					let libs: Vec<sd_core::ops::libraries::list::output::LibraryInfo> = core
						.query(&sd_core::ops::libraries::list::query::ListLibrariesQuery::basic())
						.await?;
					let lib = libs.get(0).ok_or_else(|| anyhow::anyhow!("No libraries found"))?;
					let out: Option<sd_core::ops::jobs::info::output::JobInfoOutput> = core.query(&sd_core::ops::jobs::info::query::JobInfoQuery { library_id: lib.id, job_id }).await?;
					match (cli.format, out) {
						(OutputFormat::Human, Some(j)) => println!("{} {} {}% {:?}", j.id, j.name, (j.progress * 100.0) as u32, j.status),
						(OutputFormat::Json, Some(j)) => println!("{}", serde_json::to_string_pretty(&j)?),
						(_, None) => println!("Job not found"),
					}
				}
			}
		}
	}

	Ok(())
}
