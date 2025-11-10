//! Query to get a single file by ID with all related data

use crate::infra::query::{QueryError, QueryResult};
use crate::{
	context::CoreContext,
	domain::{addressing::SdPath, File},
	infra::db::entities::{content_identity, entry, sidecar, tag, user_metadata_tag},
	infra::query::LibraryQuery,
};
use sea_orm::{
	ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, JoinType, QueryFilter,
	QuerySelect, RelationTrait,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use uuid::Uuid;

/// Query to get a file by its ID with all related data
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FileByIdQuery {
	pub file_id: Uuid,
}

impl FileByIdQuery {
	pub fn new(file_id: Uuid) -> Self {
		Self { file_id }
	}
}

impl LibraryQuery for FileByIdQuery {
	type Input = FileByIdQuery;
	type Output = Option<File>;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(input)
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
			.ok_or_else(|| QueryError::Internal("Library not found".to_string()))?;

		let db = library.db();

		// Get the entry
		let entry_model = entry::Entity::find()
			.filter(entry::Column::Uuid.eq(self.file_id))
			.one(db.conn())
			.await?
			.ok_or_else(|| QueryError::Internal("File not found".to_string()))?;

		// Only proceed if this is actually a file (not a directory)
		if entry_model.kind == 1 {
			return Ok(None);
		}

		// Create placeholder SdPath
		// TODO: Resolve actual path from database
		let sd_path = SdPath::Physical {
			device_slug: format!("placeholder-{}", Uuid::new_v4()),
			path: format!("/{}", entry_model.name).into(),
		};

		// Fetch content identity and sidecars if file has content_id
		let (content_identity_domain, sidecars) = if let Some(content_id) = entry_model.content_id {
			// Get content identity
			if let Some(content_identity_model) = content_identity::Entity::find_by_id(content_id)
				.one(db.conn())
				.await?
			{
				let content_uuid = content_identity_model.uuid;

				// Fetch sidecars for this content UUID
				let sidecars = if let Some(uuid) = content_uuid {
					sidecar::Entity::find()
						.filter(sidecar::Column::ContentUuid.eq(uuid))
						.all(db.conn())
						.await?
						.into_iter()
						.map(|s| crate::domain::Sidecar {
							id: s.id,
							content_uuid: s.content_uuid,
							kind: s.kind,
							variant: s.variant,
							format: s.format,
							status: s.status,
							size: s.size,
							created_at: s.created_at,
							updated_at: s.updated_at,
						})
						.collect()
				} else {
					Vec::new()
				};

				// Convert content_identity to domain type
				let content_identity = crate::domain::ContentIdentity {
					uuid: content_identity_model.uuid.unwrap_or_else(|| Uuid::new_v4()),
					kind: crate::domain::ContentKind::from_id(content_identity_model.kind_id),
					content_hash: content_identity_model.content_hash,
					integrity_hash: content_identity_model.integrity_hash,
					mime_type_id: content_identity_model.mime_type_id,
					text_content: content_identity_model.text_content,
					total_size: content_identity_model.total_size,
					entry_count: content_identity_model.entry_count,
					first_seen_at: content_identity_model.first_seen_at,
					last_verified_at: content_identity_model.last_verified_at,
				};

				(Some(content_identity), sidecars)
			} else {
				(None, Vec::new())
			}
		} else {
			(None, Vec::new())
		};

		// Convert to File using from_entity_model
		let mut file = File::from_entity_model(entry_model, sd_path);
		file.sidecars = sidecars;
		file.content_identity = content_identity_domain;
		if let Some(ref ci) = file.content_identity {
			file.content_kind = ci.kind;
		}

		Ok(Some(file))
	}
}

// Register the query
crate::register_library_query!(FileByIdQuery, "files.by_id");
