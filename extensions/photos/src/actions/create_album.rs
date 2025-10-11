use spacedrive_sdk::{action, action_execute};

use chrono::Utc;
use spacedrive_sdk::prelude::*;
use uuid::Uuid;

use crate::models::*;

#[action]
pub async fn create_album(
	ctx: &ActionContext,
	name: String,
	photo_ids: Vec<Uuid>,
) -> ActionResult<ActionPreview> {
	Ok(ActionPreview {
		title: "Create Album".to_string(),
		description: format!("Create album '{}' with {} photos", name, photo_ids.len()),
		changes: vec![Change::CreateModel {
			model_type: "Album".to_string(),
			data: serde_json::to_value(&Album {
				id: Uuid::new_v4(),
				name: name.clone(),
				photo_ids: photo_ids.clone(),
				cover_photo_id: photo_ids.first().cloned(),
				created_at: Utc::now(),
				album_type: AlbumType::Manual,
			})?,
		}],
		reversible: true,
	})
}

#[action_execute]
pub async fn create_album_execute(
	ctx: &ActionContext,
	preview: ActionPreview,
) -> ActionResult<ExecutionResult> {
	for change in preview.changes {
		match change {
			Change::CreateModel { model_type, data } => {
				let album: Album = serde_json::from_value(data)?;
				ctx.vdfs().create_model(album).await?;
			}
			_ => {}
		}
	}

	Ok(ExecutionResult {
		success: true,
		message: "Album created successfully".to_string(),
	})
}
