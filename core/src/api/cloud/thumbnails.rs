use crate::api::{Ctx, R};

use sd_cloud_schema::{devices, libraries};
use sd_prisma::prisma::file_path::cas_id;

use futures::FutureExt;
use futures_concurrency::future::TryJoin;
use rspc::alpha::AlphaRouter;
use serde::Deserialize;
use tokio::sync::oneshot;
use tracing::{debug, error};

pub fn mount() -> AlphaRouter<Ctx> {
	R.router().procedure("get", {
		#[derive(Deserialize, specta::Type)]
		struct CloudThumbnailRequestArgs {
			device_pub_id: devices::PubId,
			library_pub_id: libraries::PubId,
			cas_id: cas_id::Type,
		}

		R.mutation(
			|node,
			 CloudThumbnailRequestArgs {
			     device_pub_id,
			     library_pub_id,
			     cas_id,
			 }: CloudThumbnailRequestArgs| async move {
				let ((client, access_token), cloud_p2p) = (
					super::get_client_and_access_token(&node),
					node.cloud_services
						.cloud_p2p()
						.map(|res| res.map_err(Into::into)),
				)
					.try_join()
					.await?;

				let (tx, rx) = oneshot::channel();

				cloud_p2p
					.request_thumbnail_data(device_pub_id, cas_id, library_pub_id, tx)
					.await;

				// Log rx output
				let out = rx.await;

				let out = out.map_err(|e| {
					error!(?e, "Failed to receive thumbnail data");
					rspc::Error::new(
						rspc::ErrorCode::InternalServerError,
						String::from("Failed to receive thumbnail data"),
					)
				})?;

				debug!(?out, "Received thumbnail data");

				Ok(())
			},
		)
	})
}
