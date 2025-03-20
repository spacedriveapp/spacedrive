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
// .procedure("request_join", {
// 	#[derive(Deserialize, specta::Type)]
// 	struct SyncGroupsRequestJoinArgs {
// 		sync_group: groups::GroupWithDevices,
// 		asking_device: devices::Device,
// 	}

// 	R.mutation(
// 		|node,
// 		 SyncGroupsRequestJoinArgs {
// 		     sync_group,
// 		     asking_device,
// 		 }: SyncGroupsRequestJoinArgs| async move {
// 			let ((client, access_token), current_device_pub_id, cloud_p2p) = (
// 				super::get_client_and_access_token(&node),
// 				node.config.get().map(|config| Ok(config.id.into())),
// 				node.cloud_services
// 					.cloud_p2p()
// 					.map(|res| res.map_err(Into::into)),
// 			)
// 				.try_join()
// 				.await?;

// 			let group_pub_id = sync_group.pub_id;

// 			debug!("My pub id: {:?}", current_device_pub_id);
// 			debug!("Asking device pub id: {:?}", asking_device.pub_id);
// 			if asking_device.pub_id != current_device_pub_id {
// 				return Err(rspc::Error::new(
// 					rspc::ErrorCode::BadRequest,
// 					String::from("Asking device must be the current device"),
// 				));
// 			}

// 			let groups::request_join::Response(existing_devices) =
// 				super::handle_comm_error(
// 					client
// 						.sync()
// 						.groups()
// 						.request_join(groups::request_join::Request {
// 							access_token,
// 							group_pub_id,
// 							current_device_pub_id,
// 						})
// 						.await,
// 					"Failed to update library;",
// 				)??;

// 			let (tx, rx) = oneshot::channel();

// 			cloud_p2p
// 				.request_join_sync_group(
// 					existing_devices,
// 					cloud_p2p::authorize_new_device_in_sync_group::Request {
// 						sync_group,
// 						asking_device,
// 					},
// 					tx,
// 				)
// 				.await;

// 			JoinedSyncGroupReceiver {
// 				node,
// 				group_pub_id,
// 				rx,
// 			}
// 			.dispatch();

// 			debug!(%group_pub_id, "Requested to join sync group");

// 			Ok(())
// 		},
// 	)
// })

// struct JoinedSyncGroupReceiver {
// 	node: Arc<Node>,
// 	group_pub_id: groups::PubId,
// 	rx: oneshot::Receiver<JoinedLibraryCreateArgs>,
// }

// impl JoinedSyncGroupReceiver {
// 	fn dispatch(self) {
// 		spawn(async move {
// 			let Self {
// 				node,
// 				group_pub_id,
// 				rx,
// 			} = self;

// 			if let Ok(JoinedLibraryCreateArgs {
// 				pub_id: libraries::PubId(pub_id),
// 				name,
// 				description,
// 			}) = rx.await
// 			{
// 				let Ok(name) =
// 					LibraryName::new(name).map_err(|e| error!(?e, "Invalid library name"))
// 				else {
// 					return;
// 				};

// 				let Ok(library) = node
// 					.libraries
// 					.create_with_uuid(pub_id, name, description, true, None, &node)
// 					.await
// 					.map_err(|e| {
// 						error!(?e, "Failed to create library from sync group join response")
// 					})
// 				else {
// 					return;
// 				};

// 				if let Err(e) = library.init_cloud_sync(&node, group_pub_id).await {
// 					error!(?e, "Failed to initialize cloud sync for library");
// 				}
// 			}
// 		});
// 	}
// }
