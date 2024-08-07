use crate::{api::utils::library, invalidate_query};

use super::*;

pub fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("get", {
			R.with2(library())
				.query(|(node, library), _: ()| async move {
					// Ok(
					// 	sd_cloud_api::library::get(node.cloud_api_config().await, library.id)
					// 		.await?,
					// )

					Ok(())
				})
		})
		.procedure("list", {
			R.query(|node, _: ()| async move {
				// Ok(sd_cloud_api::library::list(node.cloud_api_config().await).await?)
				Ok(())
			})
		})
		.procedure("create", {
			R.with2(library())
				.mutation(|(node, library), _: ()| async move {
					// let node_config = node.config.get().await;
					// let cloud_library = sd_cloud_api::library::create(
					// 	node.cloud_api_config().await,
					// 	library.id,
					// 	&library.config().await.name,
					// 	library.instance_uuid,
					// 	library.identity.to_remote_identity(),
					// 	node_config.id,
					// 	node_config.identity.to_remote_identity(),
					// 	&node.p2p.peer_metadata(),
					// )
					// .await?;
					// node.libraries
					// 	.edit(
					// 		library.id,
					// 		None,
					// 		MaybeUndefined::Undefined,
					// 		MaybeUndefined::Value(cloud_library.id),
					// 		None,
					// 	)
					// 	.await?;

					invalidate_query!(library, "cloud.library.get");

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
				Ok(())
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
