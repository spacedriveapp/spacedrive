use crate::job::{Job, JobReportUpdate, JobResult, WorkerContext};
use std::path::PathBuf;

use self::scan::ScanProgress;
mod scan;

// Re-exporting
pub use scan::*;

use scan::scan_path;

#[derive(Debug)]
pub struct IndexerJob {
	pub path: PathBuf,
}

#[async_trait::async_trait]
impl Job for IndexerJob {
	fn name(&self) -> &'static str {
		"indexer"
	}
	async fn run(&self, ctx: WorkerContext) -> JobResult {
		scan_path(&ctx.library_ctx(), &self.path, move |p| {
			ctx.progress(
				p.iter()
					.map(|p| match p.clone() {
						ScanProgress::ChunkCount(c) => JobReportUpdate::TaskCount(c),
						ScanProgress::SavedChunks(p) => JobReportUpdate::CompletedTaskCount(p),
						ScanProgress::Message(m) => JobReportUpdate::Message(m),
					})
					.collect(),
			)
		})
		.await
	}
}

// // PathContext provides the indexer with instruction to handle particular directory structures and identify rich context.
// pub struct PathContext {
// 	// an app specific key "com.github.repo"
// 	pub key: String,
// 	pub name: String,
// 	pub is_dir: bool,
// 	// possible file extensions for this path
// 	pub extensions: Vec<String>,
// 	// sub-paths that must be found
// 	pub must_contain_sub_paths: Vec<String>,
// 	// sub-paths that are ignored
// 	pub always_ignored_sub_paths: Option<String>,
// }
