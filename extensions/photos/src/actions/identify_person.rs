use spacedrive_sdk::{action, action_execute};

use spacedrive_sdk::prelude::*;
use uuid::Uuid;

use crate::models::FaceDetection;

#[action]
pub async fn identify_person(
	ctx: &ActionContext,
	face_detections: Vec<(Uuid, FaceDetection)>,
	name: String,
) -> ActionResult<ActionPreview> {
	let photo_count = face_detections.len();

	Ok(ActionPreview {
		title: "Identify Person".to_string(),
		description: format!("Identify {} photos as {}", photo_count, name),
		changes: face_detections
			.iter()
			.map(|(photo_id, _face)| Change::UpdateCustomField {
				entry_id: *photo_id,
				field: "identified_people".to_string(),
				value: serde_json::to_value(&name).unwrap(),
			})
			.collect(),
		reversible: true,
	})
}
