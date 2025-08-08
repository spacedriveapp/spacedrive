use anyhow::Result;

use crate::core_boot::CoreBoot;
use crate::metrics::ScenarioResult;
use crate::recipe::Recipe;

#[async_trait::async_trait]
pub trait Scenario {
    fn name(&self) -> &'static str;
    fn describe(&self) -> &'static str;
    async fn prepare(&mut self, boot: &CoreBoot, recipe: &Recipe) -> Result<()>;
    async fn run(&mut self, boot: &CoreBoot, recipe: &Recipe) -> Result<Vec<ScenarioResult>>;
}

pub mod indexing_discovery;
pub mod registry;

pub use indexing_discovery::IndexingDiscoveryScenario;
