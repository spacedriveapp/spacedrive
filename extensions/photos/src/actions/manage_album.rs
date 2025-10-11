use spacedrive_sdk::{action, action_execute};

use spacedrive_sdk::prelude::*;
use uuid::Uuid;

use crate::models::Album;

#[action]
pub async fn remove_photo_from_album(
	ctx: &ActionContext,
	album_id: Uuid,
	photo_id: Uuid,
) -> ActionResult<ActionPreview> {
	let album = ctx.vdfs().get_model::<Album>(album_id).await?;

	Ok(ActionPreview {
		title: "Remove from Album".to_string(),
		description: format!("Remove photo from '{}'", album.name),
		changes: vec![Change::UpdateModel {
			model_id: album_id,
			field: "photo_ids".to_string(),
			operation: "remove".to_string(),
			value: serde_json::to_value(&photo_id)?,
		}],
		reversible: true,
	})
}
