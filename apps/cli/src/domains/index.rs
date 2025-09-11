use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};

use crate::context::Context;

#[derive(Debug, Clone, ValueEnum)]
pub enum IndexModeArg { Shallow, Content, Deep }

#[derive(Debug, Clone, ValueEnum)]
pub enum IndexScopeArg { Current, Recursive }

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
pub struct IndexStartArgs {
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
pub struct QuickScanArgs {
	pub path: String,
	#[arg(long, value_enum, default_value = "current")]
	pub scope: IndexScopeArg,
}

#[derive(Parser, Debug, Clone)]
pub struct BrowseArgs {
	pub path: String,
	#[arg(long, value_enum, default_value = "current")]
	pub scope: IndexScopeArg,
	#[arg(long, default_value_t = false)]
	pub content: bool,
}

#[derive(Subcommand, Debug)]
pub enum IndexCmd {
	/// Start indexing for one or more paths
	Start(IndexStartArgs),
	/// Quick scan of a path (ephemeral)
	QuickScan(QuickScanArgs),
	/// Browse a path without adding as location
	Browse(BrowseArgs),
}

pub async fn run(ctx: &Context, cmd: IndexCmd) -> Result<()> {
	match cmd {
		IndexCmd::Start(args) => {
			use sd_core::ops::indexing::input::IndexInput;
			use sd_core::ops::indexing::job::{IndexMode, IndexPersistence, IndexScope};

			let library_id = if let Some(id) = args.library {
				id
			} else {
				let libs: Vec<sd_core::ops::libraries::list::output::LibraryInfo> = ctx
					.core
					.query(&sd_core::ops::libraries::list::query::ListLibrariesQuery::basic())
					.await?;
				match libs.len() {
					0 => anyhow::bail!("No libraries found; specify --library after creating one"),
					1 => libs[0].id,
					_ => anyhow::bail!("Multiple libraries found; please specify --library <UUID>"),
				}
			};

			let persistence = if args.persistent { IndexPersistence::Persistent } else { IndexPersistence::Ephemeral };

			let mut local_paths: Vec<std::path::PathBuf> = Vec::new();
			for s in &args.paths {
				let sd = sd_core::domain::addressing::SdPath::from_uri(s)
					.unwrap_or_else(|_| sd_core::domain::addressing::SdPath::local(s));
				if let Some(p) = sd.as_local_path() { local_paths.push(p.to_path_buf()); } else { anyhow::bail!(format!("Non-local address not supported for indexing yet: {}", s)); }
			}

			let input = IndexInput::new(library_id, local_paths)
				.with_mode(IndexMode::from(args.mode.clone()))
				.with_scope(IndexScope::from(args.scope.clone()))
				.with_include_hidden(args.include_hidden)
				.with_persistence(persistence);

			if let Err(errors) = input.validate() { anyhow::bail!(errors.join("; ")); }

			ctx.core.action(&input).await?;
			println!("Indexing request submitted");
		}
		IndexCmd::QuickScan(args) => {
			use sd_core::ops::indexing::input::IndexInput;
			use sd_core::ops::indexing::job::{IndexMode, IndexPersistence, IndexScope};
			let libs: Vec<sd_core::ops::libraries::list::output::LibraryInfo> = ctx
				.core
				.query(&sd_core::ops::libraries::list::query::ListLibrariesQuery::basic())
				.await?;
			let library_id = if libs.len() == 1 { libs[0].id } else { anyhow::bail!("Specify --library for quick-scan when multiple libraries exist") };
			let sd = sd_core::domain::addressing::SdPath::from_uri(&args.path).unwrap_or_else(|_| sd_core::domain::addressing::SdPath::local(&args.path));
			let p = sd.as_local_path().ok_or_else(|| anyhow::anyhow!("Non-local path not supported yet"))?;
			let input = IndexInput::new(library_id, vec![p.to_path_buf()])
				.with_mode(IndexMode::Shallow)
				.with_scope(IndexScope::from(args.scope.clone()))
				.with_persistence(IndexPersistence::Ephemeral);
			ctx.core.action(&input).await?;
			println!("Quick scan request submitted");
		}
		IndexCmd::Browse(args) => {
			use sd_core::ops::indexing::input::IndexInput;
			use sd_core::ops::indexing::job::{IndexMode, IndexPersistence, IndexScope};
			let libs: Vec<sd_core::ops::libraries::list::output::LibraryInfo> = ctx
				.core
				.query(&sd_core::ops::libraries::list::query::ListLibrariesQuery::basic())
				.await?;
			let library_id = if libs.len() == 1 { libs[0].id } else { anyhow::bail!("Specify --library for browse when multiple libraries exist") };
			let sd = sd_core::domain::addressing::SdPath::from_uri(&args.path).unwrap_or_else(|_| sd_core::domain::addressing::SdPath::local(&args.path));
			let p = sd.as_local_path().ok_or_else(|| anyhow::anyhow!("Non-local path not supported yet"))?;
			let input = IndexInput::new(library_id, vec![p.to_path_buf()])
				.with_mode(if args.content { IndexMode::Content } else { IndexMode::Shallow })
				.with_scope(IndexScope::from(args.scope.clone()))
				.with_persistence(IndexPersistence::Ephemeral);
			ctx.core.action(&input).await?;
			println!("Browse request submitted");
		}
	}
	Ok(())
}

