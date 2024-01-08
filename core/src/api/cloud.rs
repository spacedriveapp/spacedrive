use crate::{api::libraries::LibraryConfigWrapped, invalidate_query, library::LibraryName};

use reqwest::Response;
use rspc::alpha::AlphaRouter;
use serde::de::DeserializeOwned;

use uuid::Uuid;

use super::{utils::library, Ctx, R};

#[allow(unused)]
async fn parse_json_body<T: DeserializeOwned>(response: Response) -> Result<T, rspc::Error> {
	response.json().await.map_err(|_| {
		rspc::Error::new(
			rspc::ErrorCode::InternalServerError,
			"JSON conversion failed".to_string(),
		)
	})
}

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.merge("library.", library::mount())
		.procedure("getApiOrigin", {
			R.query(|node, _: ()| async move { Ok(node.env.api_url.lock().await.to_string()) })
		})
		.procedure("setApiOrigin", {
			R.mutation(|node, origin: String| async move {
				let mut origin_env = node.env.api_url.lock().await;
				*origin_env = origin;

				node.config.write(|c| c.auth_token = None).await.ok();

				Ok(())
			})
		})
}

mod library {

	use super::*;

	pub fn mount() -> AlphaRouter<Ctx> {
		R.router()
			.procedure("get", {
				R.with2(library())
					.query(|(node, library), _: ()| async move {
						Ok(
							sd_cloud_api::library::get(node.cloud_api_config().await, library.id)
								.await?,
						)
					})
			})
			.procedure("list", {
				R.query(|node, _: ()| async move {
					Ok(sd_cloud_api::library::list(node.cloud_api_config().await).await?)
				})
			})
			.procedure("create", {
				R.with2(library())
					.mutation(|(node, library), _: ()| async move {
						sd_cloud_api::library::create(
							node.cloud_api_config().await,
							library.id,
							&library.config().await.name,
							library.instance_uuid,
							&library.identity.to_remote_identity(),
						)
						.await?;

						invalidate_query!(library, "cloud.library.get");

						Ok(())
					})
			})
			.procedure("join", {
				R.mutation(|node, library_id: Uuid| async move {
					let Some(cloud_library) =
						sd_cloud_api::library::get(node.cloud_api_config().await, library_id)
							.await?
					else {
						return Err(rspc::Error::new(
							rspc::ErrorCode::NotFound,
							"Library not found".to_string(),
						));
					};

					let library = node
						.libraries
						.create_with_uuid(
							library_id,
							LibraryName::new(cloud_library.name).map_err(|e| {
								rspc::Error::new(
									rspc::ErrorCode::InternalServerError,
									e.to_string(),
								)
							})?,
							None,
							false,
							None,
							&node,
						)
						.await?;

					sd_cloud_api::library::join(
						node.cloud_api_config().await,
						library_id,
						library.instance_uuid,
						&library.identity.to_remote_identity(),
					)
					.await?;

					invalidate_query!(library, "cloud.library.get");
					invalidate_query!(library, "cloud.library.list");

					Ok(LibraryConfigWrapped::from_library(&library).await)
				})
			})
	}
}
