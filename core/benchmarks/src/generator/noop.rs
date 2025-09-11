use anyhow::Result;

use super::DatasetGenerator;
use crate::recipe::Recipe;

#[derive(Debug, Default)]
pub struct NoopGenerator;

#[async_trait::async_trait]
impl DatasetGenerator for NoopGenerator {
	fn name(&self) -> &'static str {
		"noop"
	}
	async fn generate(&self, _recipe: &Recipe) -> Result<()> {
		Ok(())
	}
}
