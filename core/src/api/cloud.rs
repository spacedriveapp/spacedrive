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
		.merge("locations.", locations::mount())
		.procedure("getApiOrigin", {
			R.query(|node, _: ()| async move { Ok(node.env.api_url.lock().await.to_string()) })
		})
		.procedure("setApiOrigin", {
			R.mutation(|node, origin: String| async move {
				let mut origin_env = node.env.api_url.lock().await;
				origin_env.clone_from(&origin);

				node.config
					.write(|c| {
						c.auth_token = None;
						c.sd_api_origin = Some(origin);
					})
					.await
					.ok();

				Ok(())
			})
		})
}

mod library {
	use std::str::FromStr;

	use sd_p2p::RemoteIdentity;

	use crate::util::MaybeUndefined;

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
						let node_config = node.config.get().await;
						let cloud_library = sd_cloud_api::library::create(
							node.cloud_api_config().await,
							library.id,
							&library.config().await.name,
							library.instance_uuid,
							library.identity.to_remote_identity(),
							node_config.id,
							node_config.identity.to_remote_identity(),
							&node.p2p.peer_metadata(),
						)
						.await?;
						node.libraries
							.edit(
								library.id,
								None,
								MaybeUndefined::Undefined,
								MaybeUndefined::Value(cloud_library.id),
								None,
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
							true,
						)
						.await?;
					node.libraries
						.edit(
							library.id,
							None,
							MaybeUndefined::Undefined,
							MaybeUndefined::Value(cloud_library.id),
							None,
						)
						.await?;

					let node_config = node.config.get().await;
					let instances = sd_cloud_api::library::join(
						node.cloud_api_config().await,
						library_id,
						library.instance_uuid,
						library.identity.to_remote_identity(),
						node_config.id,
						node_config.identity.to_remote_identity(),
						node.p2p.peer_metadata(),
					)
					.await?;

					for instance in instances {
						crate::cloud::sync::receive::upsert_instance(
							library.id,
							&library.db,
							&library.sync,
							&node.libraries,
							&instance.uuid,
							instance.identity,
							&instance.node_id,
							RemoteIdentity::from_str(&instance.node_remote_identity)
								.expect("malformed remote identity in the DB"),
							instance.metadata,
						)
						.await?;
					}

					invalidate_query!(library, "cloud.library.get");
					invalidate_query!(library, "cloud.library.list");

					Ok(LibraryConfigWrapped::from_library(&library).await)
				})
			})
			.procedure("sync", {
				R.with2(library())
					.mutation(|(_, library), _: ()| async move {
						library.do_cloud_sync();
						Ok(())
					})
			})
	}
}
mod locations {
	use super::*;
	use http_body::Full;
	use once_cell::sync::OnceCell;
	use serde::{Deserialize, Serialize};
	use specta::Type;
	#[derive(Type, Serialize, Deserialize)]
	pub struct CloudLocation {
		id: String,
		name: String,
	}

	pub fn mount() -> AlphaRouter<Ctx> {
		R.router()
			.procedure("list", {
				R.query(|node, _: ()| async move {
					sd_cloud_api::locations::list(node.cloud_api_config().await)
						.await
						.map_err(Into::into)
				})
			})
			.procedure("create", {
				R.mutation(|node, name: String| async move {
					sd_cloud_api::locations::create(node.cloud_api_config().await, name)
						.await
						.map_err(Into::into)
				})
			})
			.procedure("remove", {
				R.mutation(|node, id: String| async move {
					sd_cloud_api::locations::create(node.cloud_api_config().await, id)
						.await
						.map_err(Into::into)
				})
			})
	}
}
