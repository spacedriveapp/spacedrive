//! Read the rows of one model a loaded extension keeps in its own store.
//!
//! An extension's jobs write models (a person, a per-file summary) into
//! `<library>/extensions/<extension id>/data.db`. Its client views read them
//! back through this query, so a view needs no host side of its own. Only
//! models the extension declared are readable, and the store is never
//! created here: a library the extension never ran in answers no rows.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
	context::CoreContext,
	infra::{
		api::SessionContext,
		query::{LibraryQuery, QueryError, QueryResult},
	},
};

/// The most rows one read answers with.
const ROW_CAP: usize = 10_000;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ExtensionModelsInput {
	pub extension_id: String,
	/// A model name the extension registered, such as `Person`
	pub model: String,
	#[serde(default)]
	pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ExtensionModelsOutput {
	/// Each row as the extension wrote it, newest first
	pub rows: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionModelsQuery {
	input: ExtensionModelsInput,
}

impl LibraryQuery for ExtensionModelsQuery {
	type Input = ExtensionModelsInput;
	type Output = ExtensionModelsOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: SessionContext,
	) -> QueryResult<Self::Output> {
		#[cfg(feature = "wasm")]
		{
			let library_id = session
				.current_library_id
				.ok_or_else(|| QueryError::Internal("No library in session".to_string()))?;
			let library = context
				.libraries()
				.await
				.get_library(library_id)
				.await
				.ok_or_else(|| QueryError::Internal("Library not found".to_string()))?;
			let plugin_manager = context
				.get_plugin_manager()
				.await
				.ok_or_else(|| QueryError::Internal("extensions are not initialized".into()))?;
			let pm = plugin_manager.read().await;
			if pm.get_manifest(&self.input.extension_id).await.is_none() {
				return Err(QueryError::Internal(format!(
					"extension {} is not loaded",
					self.input.extension_id
				)));
			}
			let schema = pm.model_registry().schema_for(&self.input.extension_id);
			drop(pm);
			if !schema.models.contains_key(&self.input.model) {
				return Err(QueryError::Internal(format!(
					"{} declares no model {}",
					self.input.extension_id, self.input.model
				)));
			}
			let store_dir = library
				.path()
				.join("extensions")
				.join(&self.input.extension_id);
			if !tokio::fs::try_exists(&store_dir).await.unwrap_or(false) {
				return Ok(ExtensionModelsOutput { rows: Vec::new() });
			}
			let db = crate::infra::extension::open_extension_store(
				library.path(),
				&self.input.extension_id,
				&schema,
			)
			.await
			.map_err(|e| QueryError::Internal(format!("open extension store: {e}")))?;
			let limit = self
				.input
				.limit
				.map(|l| l as usize)
				.unwrap_or(ROW_CAP)
				.min(ROW_CAP);
			let rows = db
				.facet_rows(&self.input.model, None, limit)
				.await
				.map_err(|e| QueryError::Internal(e.to_string()))?;
			Ok(ExtensionModelsOutput { rows })
		}
		#[cfg(not(feature = "wasm"))]
		{
			let _ = (context, session);
			Err(QueryError::Internal(
				"this build has no extension runtime".into(),
			))
		}
	}
}

crate::register_library_query!(ExtensionModelsQuery, "extensions.models");
