use clap::{Args, ValueEnum};
use std::path::PathBuf;
use uuid::Uuid;

use sd_core::{
	domain::addressing::SdPath,
	ops::core::ephemeral_status::EphemeralCacheStatusInput,
	ops::indexing::{input::IndexInput, job::IndexScope},
};

#[derive(Debug, Clone, ValueEnum)]
pub enum IndexScopeArg {
	Current,
	Recursive,
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

		Ok(IndexInput::new(library_id, local_paths)
			.with_scope(IndexScope::from(self.scope.clone()))
			.with_include_hidden(self.include_hidden))
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
			.with_scope(IndexScope::from(self.scope.clone())))
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
			.with_scope(IndexScope::from(self.scope.clone())))
	}
}

/// Arguments for ephemeral cache status
#[derive(Args, Debug, Clone)]
pub struct EphemeralCacheArgs {
	/// Filter by path substring
	#[arg(long)]
	pub filter: Option<String>,

	/// Show detailed memory breakdown
	#[arg(long, default_value_t = false)]
	pub detailed: bool,
}

impl EphemeralCacheArgs {
	pub fn to_input(&self) -> EphemeralCacheStatusInput {
		EphemeralCacheStatusInput {
			path_filter: self.filter.clone(),
			detailed: self.detailed,
		}
	}
}
