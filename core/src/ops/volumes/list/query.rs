//! Volume list query

use super::output::VolumeListOutput;
use crate::{
	context::CoreContext,
	infra::{
		db::entities,
		query::{LibraryQuery, QueryError, QueryResult},
	},
	volume::VolumeFingerprint,
};
use sea_orm::EntityTrait;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VolumeListQueryInput {}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VolumeListQuery;

impl LibraryQuery for VolumeListQuery {
	type Input = VolumeListQueryInput;
	type Output = VolumeListOutput;

	fn from_input(_input: Self::Input) -> QueryResult<Self> {
		Ok(Self {})
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let library_id = session
			.current_library_id
			.ok_or_else(|| QueryError::Internal("No library selected".to_string()))?;

		let library = context
			.libraries()
			.await
			.get_library(library_id)
			.await
			.ok_or_else(|| QueryError::Internal("Library not found".to_string()))?;

		let db = library.db().conn();

		let volumes = entities::volume::Entity::find()
			.all(db)
			.await?;

		let volume_items = volumes
			.into_iter()
			.map(|v| super::output::VolumeItem {
				uuid: v.uuid,
				name: v.display_name.unwrap_or_else(|| "Unnamed".to_string()),
				fingerprint: VolumeFingerprint(v.fingerprint),
				volume_type: v.volume_type.unwrap_or_else(|| "Unknown".to_string()),
			})
			.collect();

		Ok(VolumeListOutput {
			volumes: volume_items,
		})
	}
}

crate::register_library_query!(VolumeListQuery, "volumes.list");
