use crate::{api::utils::library, invalidate_query};
use rspc::alpha::AlphaRouter;
use sd_cloud_schema::libraries;
use tracing::debug;
use uuid::Uuid;

use crate::{
	api::{Ctx, R},
	try_get_cloud_services_client,
};

pub fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("get", {
			R.query(|node, req: libraries::get::Request| async move {
				let libraries::get::Response(library) = super::handle_comm_error(
					try_get_cloud_services_client!(node)?
						.libraries()
						.get(req)
						.await,
					"Failed to get library;",
				)??;

				debug!(?library, "Got library");

				Ok(library)
			})
		})
		.procedure("list", {
			R.query(|node, req: libraries::list::Request| async move {
				let libraries::list::Response(libraries) = super::handle_comm_error(
					try_get_cloud_services_client!(node)?
						.libraries()
						.list(req)
						.await,
					"Failed to list libraries;",
				)??;

				debug!(?libraries, "Listed libraries");

				Ok(libraries)
			})
		})
		.procedure("create", {
			#[derive(Debug, serde::Serialize, serde::Deserialize, specta::Type)]
			struct LibrariesCreateArgs {
				access_token: AccessToken,
				device_pub_id: devices::PubId,
			}

			R.with2(library())
				.mutation(|(node, library), args: LibrariesCreateArgs | async move {
					let req = libraries::create::Request {
						name: library.config().await.name.to_string(),
						access_token: args.access_token,
						pub_id: library.id,
						device_pub_id: args.device_pub_id,
					};
					super::handle_comm_error(
						try_get_cloud_services_client!(node)?
							.libraries()
							.create(req)
							.await,
						"Failed to create library;",
					)??;

					Ok(())
				})
		})
		.procedure("join", {
			R.mutation(|node, library_id: Uuid| async move {
				// let Some(cloud_library) =
				// 	sd_cloud_api::library::get(node.cloud_api_config().await, library_id).await?
				// else {
				// 	return Err(rspc::Error::new(
				// 		rspc::ErrorCode::NotFound,
				// 		"Library not found".to_string(),
				// 	));
				// };

				// let library = node
				// 	.libraries
				// 	.create_with_uuid(
				// 		library_id,
				// 		LibraryName::new(cloud_library.name).map_err(|e| {
				// 			rspc::Error::new(rspc::ErrorCode::InternalServerError, e.to_string())
				// 		})?,
				// 		None,
				// 		false,
				// 		None,
				// 		&node,
				// 		true,
				// 	)
				// 	.await?;
				// node.libraries
				// 	.edit(
				// 		library.id,
				// 		None,
				// 		MaybeUndefined::Undefined,
				// 		MaybeUndefined::Value(cloud_library.id),
				// 		None,
				// 	)
				// 	.await?;

				// let node_config = node.config.get().await;
				// let instances = sd_cloud_api::library::join(
				// 	node.cloud_api_config().await,
				// 	library_id,
				// 	library.instance_uuid,
				// 	library.identity.to_remote_identity(),
				// 	node_config.id,
				// 	node_config.identity.to_remote_identity(),
				// 	node.p2p.peer_metadata(),
				// )
				// .await?;

				// for instance in instances {
				// 	crate::cloud::sync::receive::upsert_instance(
				// 		library.id,
				// 		&library.db,
				// 		&library.sync,
				// 		&node.libraries,
				// 		&instance.uuid,
				// 		instance.identity,
				// 		&instance.node_id,
				// 		RemoteIdentity::from_str(&instance.node_remote_identity)
				// 			.expect("malformed remote identity in the DB"),
				// 		instance.metadata,
				// 	)
				// 	.await?;
				// }

				// invalidate_query!(library, "cloud.library.get");
				// invalidate_query!(library, "cloud.library.list");

				// Ok(LibraryConfigWrapped::from_library(&library).await)

				debug!("TODO: Functionality not implemented. Joining will be removed in the future, but for now, it's a no-op");

				Ok(())
			})
		})
		.procedure("update", {
			R.mutation(|node, req: libraries::update::Request| async move {
				super::handle_comm_error(
					try_get_cloud_services_client!(node)?
						.libraries()
						.update(req)
						.await,
					"Failed to update library;",
				)??;

				debug!("Updated library");

				Ok(())
			})
		})
}
