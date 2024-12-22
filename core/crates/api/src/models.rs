use rspc::alpha::AlphaRouter;

use super::{Ctx, R};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router().procedure("image_detection.list", {
		R.query(
			|_, _: ()| -> std::result::Result<Vec<&'static str>, rspc::Error> {
				#[cfg(not(feature = "ai"))]
				return Err(rspc::Error::new(
					rspc::ErrorCode::MethodNotSupported,
					"AI feature is not available".to_string(),
				));

				#[cfg(feature = "ai")]
				{
					use sd_ai::old_image_labeler::{Model, YoloV8};
					Ok(YoloV8::versions())
				}
			},
		)
	})
}
