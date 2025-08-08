use crate::recipe::Recipe;

#[async_trait::async_trait]
pub trait DatasetGenerator {
	fn name(&self) -> &'static str;
	async fn generate(&self, recipe: &Recipe) -> anyhow::Result<()>;
}

pub mod filesystem;
pub mod noop;
pub mod registry;

pub use filesystem::FileSystemGenerator;
pub use noop::NoopGenerator;
