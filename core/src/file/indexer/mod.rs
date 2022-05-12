use crate::job::{
  jobs::{Job, JobReportUpdate},
  worker::WorkerContext,
};
use anyhow::Result;

use self::scan::ScanProgress;
pub mod pathctx;
pub mod scan;

pub use {pathctx::PathContext, scan::scan_path};

#[derive(Debug)]
pub struct IndexerJob {
  pub path: String,
}

#[async_trait::async_trait]
impl Job for IndexerJob {
  fn name(&self) -> &'static str {
    "indexer"
  }
  async fn run(&self, ctx: WorkerContext) -> Result<()> {
    let core_ctx = ctx.core_ctx.clone();
    scan_path(&core_ctx, self.path.as_str(), move |p| {
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
    .await?;
    Ok(())
  }
}
