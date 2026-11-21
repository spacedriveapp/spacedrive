use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use super::{action::local_path, sidecar::read_shots, ShotList};
use crate::{
	context::CoreContext,
	domain::{sidecar::content_uuid_of_file, SdPath},
	infra::query::{LibraryQuery, QueryError, QueryResult},
};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ShotListInput {
	pub path: SdPath,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ShotListOutput {
	pub content_uuid: Uuid,
	/// `None` until shot detection has run on this content.
	pub shots: Option<ShotList>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShotListQuery {
	input: ShotListInput,
}

impl LibraryQuery for ShotListQuery {
	type Input = ShotListInput;
	type Output = ShotListOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let library_id = session
			.current_library_id
			.ok_or_else(|| QueryError::Internal("No library in session".to_string()))?;
		let library = context
			.libraries()
			.await
			.get_library(library_id)
			.await
			.ok_or(QueryError::LibraryNotFound(library_id))?;
		let path = local_path(&self.input.path).map_err(QueryError::InvalidInput)?;
		let content_uuid = content_uuid_of_file(&path).await.map_err(|error| {
			QueryError::InvalidInput(format!("could not read the file: {error}"))
		})?;
		let shots = read_shots(library.path(), content_uuid)
			.await
			.map_err(|error| {
				QueryError::Internal(format!("could not read the shot list: {error}"))
			})?;
		Ok(ShotListOutput {
			content_uuid,
			shots,
		})
	}
}

crate::register_library_query!(ShotListQuery, "media.shots.list");
