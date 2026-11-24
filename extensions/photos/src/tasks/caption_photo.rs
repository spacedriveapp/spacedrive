use spacedrive_sdk::task;

use spacedrive_sdk::prelude::*;

use crate::models::Caption;

#[task(requires_capability = "gpu_optional")]
pub async fn caption_photo(ctx: TaskContext, photo: Record) -> TaskResult<Caption> {
	let image_bytes = photo.read().await?;
	let text = ctx
		.ai()
		.from_registered("image_caption:photos")
		.caption(&image_bytes)
		.await?;
	Ok(Caption { version: 1, text })
}
