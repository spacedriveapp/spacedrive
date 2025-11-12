//! Model management queries

use super::{types::ModelInfo, whisper::WhisperModelManager};
use crate::{context::CoreContext, infra::query::CoreQuery};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use specta::Type;

// ============================================================================
// List Whisper Models Query
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ListWhisperModelsInput {}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ListWhisperModelsOutput {
	pub models: Vec<ModelInfo>,
	pub total_downloaded_size: u64,
}

pub struct ListWhisperModelsQuery;

impl CoreQuery for ListWhisperModelsQuery {
	type Input = ListWhisperModelsInput;
	type Output = ListWhisperModelsOutput;

	fn from_input(_input: Self::Input) -> crate::infra::query::QueryResult<Self> {
		Ok(Self)
	}

	async fn execute(
		self,
		_context: std::sync::Arc<CoreContext>,
		_session: crate::infra::api::SessionContext,
	) -> crate::infra::query::QueryResult<Self::Output> {
		let data_dir = crate::config::default_data_dir()?;
		let manager = WhisperModelManager::new(&data_dir);

		let models = manager.list_models().await?;
		let total_size = manager.total_downloaded_size().await;

		Ok(ListWhisperModelsOutput {
			models,
			total_downloaded_size: total_size,
		})
	}
}

crate::register_core_query!(ListWhisperModelsQuery, "models.whisper.list");
