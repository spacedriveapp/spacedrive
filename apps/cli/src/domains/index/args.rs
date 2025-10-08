use clap::{Args, ValueEnum};
use std::path::PathBuf;
use uuid::Uuid;

use sd_core::{
	domain::addressing::SdPath,
	ops::indexing::{
		input::IndexInput,
		job::{IndexMode, IndexPersistence, IndexScope},
		verify::input::IndexVerifyInput,
	},
};

#[derive(Debug, Clone, ValueEnum)]
pub enum IndexModeArg {
	Shallow,
	Content,
	Deep,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum IndexScopeArg {
	Current,
	Recursive,
}

impl From<IndexModeArg> for IndexMode {
	fn from(m: IndexModeArg) -> Self {
		match m {
			IndexModeArg::Shallow => Self::Shallow,
			IndexModeArg::Content => Self::Content,
			IndexModeArg::Deep => Self::Deep,
		}
	}
}

impl From<IndexScopeArg> for IndexScope {
	fn from(s: IndexScopeArg) -> Self {
		match s {
			IndexScopeArg::Current => Self::Current,
			IndexScopeArg::Recursive => Self::Recursive,
		}
	}
}

#[derive(Args, Debug, Clone)]
pub struct IndexStartArgs {
	/// Addresses to index (SdPath URIs or local paths)
	pub paths: Vec<String>,

	/// Library ID to run indexing in (defaults to the only library if just one exists)
	#[arg(long)]
	pub library: Option<Uuid>,

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

impl IndexStartArgs {
	pub fn to_input(&self, library_id: Uuid) -> anyhow::Result<IndexInput> {
		let mut local_paths: Vec<PathBuf> = Vec::new();
		for s in &self.paths {
			let sd = SdPath::from_uri(s).unwrap_or_else(|_| SdPath::local(s));
			if let Some(p) = sd.as_local_path() {
				local_paths.push(p.to_path_buf());
			} else {
				anyhow::bail!("Non-local address not supported for indexing yet: {}", s);
			}
		}

		let persistence = if self.persistent {
			IndexPersistence::Persistent
		} else {
			IndexPersistence::Ephemeral
		};

		Ok(IndexInput::new(library_id, local_paths)
			.with_mode(IndexMode::from(self.mode.clone()))
			.with_scope(IndexScope::from(self.scope.clone()))
			.with_include_hidden(self.include_hidden)
			.with_persistence(persistence))
	}
}

#[derive(Args, Debug, Clone)]
pub struct QuickScanArgs {
	pub path: String,
	#[arg(long, value_enum, default_value = "current")]
	pub scope: IndexScopeArg,
}

impl QuickScanArgs {
	pub fn to_input(&self, library_id: Uuid) -> anyhow::Result<IndexInput> {
		let sd = SdPath::from_uri(&self.path).unwrap_or_else(|_| SdPath::local(&self.path));
		let p = sd
			.as_local_path()
			.ok_or_else(|| anyhow::anyhow!("Non-local path not supported yet"))?;
		Ok(IndexInput::new(library_id, vec![p.to_path_buf()])
			.with_mode(IndexMode::Shallow)
			.with_scope(IndexScope::from(self.scope.clone()))
			.with_persistence(IndexPersistence::Ephemeral))
	}
}

#[derive(Args, Debug, Clone)]
pub struct BrowseArgs {
	pub path: String,
	#[arg(long, value_enum, default_value = "current")]
	pub scope: IndexScopeArg,
	#[arg(long, default_value_t = false)]
	pub content: bool,
}

impl BrowseArgs {
	pub fn to_input(&self, library_id: Uuid) -> anyhow::Result<IndexInput> {
		let sd = SdPath::from_uri(&self.path).unwrap_or_else(|_| SdPath::local(&self.path));
		let p = sd
			.as_local_path()
			.ok_or_else(|| anyhow::anyhow!("Non-local path not supported yet"))?;
		Ok(IndexInput::new(library_id, vec![p.to_path_buf()])
			.with_mode(if self.content {
				IndexMode::Content
			} else {
				IndexMode::Shallow
			})
			.with_scope(IndexScope::from(self.scope.clone()))
			.with_persistence(IndexPersistence::Ephemeral))
	}
}

#[derive(Args, Debug, Clone)]
pub struct IndexVerifyArgs {
	/// Path to verify (can be location root or subdirectory)
	pub path: PathBuf,

	/// Verify content hashes (slower but more thorough)
	#[arg(long, default_value_t = false)]
	pub verify_content: bool,

	/// Show detailed file-by-file comparison
	#[arg(long, default_value_t = true)]
	pub detailed: bool,

	/// Automatically fix issues (not yet implemented)
	#[arg(long, default_value_t = false)]
	pub auto_fix: bool,
}

impl IndexVerifyArgs {
	pub fn to_input(&self) -> IndexVerifyInput {
		IndexVerifyInput {
			path: self.path.clone(),
			verify_content: self.verify_content,
			detailed_report: self.detailed,
			auto_fix: self.auto_fix,
		}
	}
}
