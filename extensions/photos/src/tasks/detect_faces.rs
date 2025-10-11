use spacedrive_sdk::task;

use spacedrive_sdk::prelude::*;

use crate::models::{BoundingBox, FaceDetection};

#[task(retries = 2, timeout_ms = 30000, requires_capability = "gpu_optional")]
pub async fn detect_faces_in_photo(
	ctx: TaskContext,
	photo: Entry,
) -> TaskResult<Vec<FaceDetection>> {
	let image_bytes = photo.read().await?;

	let detections = ctx
		.ai()
		.from_registered("face_detection:photos_v1")
		.detect_faces(&image_bytes)
		.await?;

	// Convert SDK FaceDetection to our model's FaceDetection
	let converted = detections
		.into_iter()
		.map(|d| FaceDetection {
			bbox: BoundingBox {
				x: d.bbox.x,
				y: d.bbox.y,
				width: d.bbox.width,
				height: d.bbox.height,
			},
			confidence: d.confidence,
			embedding: d.embedding,
			identified_as: None,
		})
		.collect();

	Ok(converted)
}
