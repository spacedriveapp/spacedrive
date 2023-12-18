use rspc::alpha::AlphaRouter;

use super::{Ctx, R};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("image_detection.list", {
			R.query(
				|_, _: ()| -> std::result::Result<Vec<&'static str>, rspc::Error> {
					#[cfg(not(feature = "skynet"))]
					return Err(rspc::Error::new(
						rspc::ErrorCode::MethodNotSupported,
						"AI feature is not aviailable".to_string(),
					));

					#[cfg(feature = "skynet")]
					{
						use sd_skynet::image_labeler::{Model, YoloV8};
						Ok(YoloV8::versions())
					}
				},
			)
		})
		.procedure("image_detection.change", {
			R.mutation(
				#[allow(unused_variables)]
				|node, model_version: String| async move {
					#[cfg(not(feature = "skynet"))]
					return Err(rspc::Error::new(
						rspc::ErrorCode::MethodNotSupported,
						"AI feature is not aviailable".to_string(),
					)) as Result<(), rspc::Error>;

					#[cfg(feature = "skynet")]
					{
						use sd_skynet::image_labeler::YoloV8;
						use tracing::error;

						let model =
							YoloV8::model(Some(&model_version), node.data_dir.join("models"))
								.await
								.map_err(|e| {
									error!(
								"Failed to download image_detection model: '{}'; Error: {e:#?}",
								&model_version,
							);
									rspc::Error::new(
										rspc::ErrorCode::BadRequest,
										"Failed to download choosen image detection model"
											.to_string(),
									)
								})?;

						node.image_labeller.change_model(model).await.map_err(|e| {
							error!(
								"Failed to change image_detection model: '{}'; Error: {e:#?}",
								&model_version,
							);
							rspc::Error::new(
								rspc::ErrorCode::BadRequest,
								"Failed to change image detection model".to_string(),
							)
						})?;

						Ok(())
					}
				},
			)
		})
}
