use crate::{
	api::{utils::library, Ctx, R},
	library::LibraryName,
	Node,
};

use sd_core_cloud_services::JoinedLibraryCreateArgs;

use sd_cloud_schema::{
	cloud_p2p, devices, libraries,
	sync::{groups, KeyHash},
};
use sd_crypto::{cloud::secret_key::SecretKey, CryptoRng, SeedableRng};

use std::sync::Arc;

use futures::FutureExt;
use futures_concurrency::future::TryJoin;
use rspc::alpha::AlphaRouter;
use serde::{Deserialize, Serialize};
use tokio::{spawn, sync::oneshot};
use tracing::{debug, error};

pub fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("create", {
			R.with2(library())
				.mutation(|(node, library), _: ()| async move {
					use groups::create::{Request, Response};

					let ((client, access_token), device_pub_id, mut rng, key_manager) = (
						super::get_client_and_access_token(&node),
						node.config.get().map(|config| Ok(config.id.into())),
						node.master_rng
							.lock()
							.map(|mut rng| Ok(CryptoRng::from_seed(rng.generate_fixed()))),
						node.cloud_services
							.key_manager()
							.map(|res| res.map_err(Into::into)),
					)
						.try_join()
						.await?;

					let new_key = SecretKey::generate(&mut rng);
					let key_hash = KeyHash(blake3::hash(new_key.as_ref()).to_hex().to_string());

					let Response(group_pub_id) = super::handle_comm_error(
						client
							.sync()
							.groups()
							.create(Request {
								access_token: access_token.clone(),
								key_hash: key_hash.clone(),
								library_pub_id: libraries::PubId(library.id),
								device_pub_id,
							})
							.await,
						"Failed to create sync group;",
					)??;

					if let Err(e) = key_manager
						.add_key_with_hash(group_pub_id, new_key, key_hash.clone(), &mut rng)
						.await
					{
						super::handle_comm_error(
							client
								.sync()
								.groups()
								.delete(groups::delete::Request {
									access_token,
									pub_id: group_pub_id,
								})
								.await,
							"Failed to delete sync group after we failed to store secret key in key manager;",
						)??;

						return Err(e.into());
					}

					library.init_cloud_sync(&node, group_pub_id).await?;

					debug!(%group_pub_id, ?key_hash, "Created sync group");

					Ok(())
				})
		})
		.procedure("delete", {
			R.mutation(|node, pub_id: groups::PubId| async move {
				use groups::delete::Request;

				let (client, access_token) = super::get_client_and_access_token(&node).await?;

				super::handle_comm_error(
					client
						.sync()
						.groups()
						.delete(Request {
							access_token,
							pub_id,
						})
						.await,
					"Failed to delete sync group;",
				)??;

				debug!(%pub_id, "Deleted sync group");

				Ok(())
			})
		})
		.procedure("get", {
			#[derive(Deserialize, specta::Type)]
			struct CloudGetSyncGroupArgs {
				pub pub_id: groups::PubId,
				pub kind: groups::get::RequestKind,
			}

			// This is a compatibility layer because quic-rpc uses bincode for serialization
			// and bincode doesn't support serde's tagged enums, and we need them for serializing
			// to frontend
			#[derive(Debug, Serialize, specta::Type)]
			#[serde(tag = "kind", content = "data")]
			pub enum CloudSyncGroupGetResponseKind {
				WithDevices(groups::GroupWithDevices),
				FullData(groups::Group),
			}

			impl From<groups::get::ResponseKind> for CloudSyncGroupGetResponseKind {
				fn from(kind: groups::get::ResponseKind) -> Self {
					match kind {
						groups::get::ResponseKind::WithDevices(data) => {
							CloudSyncGroupGetResponseKind::WithDevices(data)
						}

						groups::get::ResponseKind::FullData(data) => {
							CloudSyncGroupGetResponseKind::FullData(data)
						}
						groups::get::ResponseKind::DevicesConnectionIds(_) => {
							unreachable!(
								"DevicesConnectionIds response is not expected, as we requested it"
							);
						}
					}
				}
			}

			R.query(
				|node, CloudGetSyncGroupArgs { pub_id, kind }: CloudGetSyncGroupArgs| async move {
					use groups::get::{Request, Response};

					let (client, access_token) = super::get_client_and_access_token(&node).await?;

					if matches!(kind, groups::get::RequestKind::DevicesConnectionIds) {
						return Err(rspc::Error::new(
							rspc::ErrorCode::PreconditionFailed,
							"This request isn't allowed here".into(),
						));
					}

					let Response(response_kind) = super::handle_comm_error(
						client
							.sync()
							.groups()
							.get(Request {
								access_token,
								pub_id,
								kind,
							})
							.await,
						"Failed to get sync group;",
					)??;

					debug!(?response_kind, "Got sync group");

					Ok(CloudSyncGroupGetResponseKind::from(response_kind))
				},
			)
		})
		.procedure("leave", {
			R.query(|node, pub_id: groups::PubId| async move {
				let ((client, access_token), current_device_pub_id, mut rng, key_manager) = (
					super::get_client_and_access_token(&node),
					node.config.get().map(|config| Ok(config.id.into())),
					node.master_rng
						.lock()
						.map(|mut rng| Ok(CryptoRng::from_seed(rng.generate_fixed()))),
					node.cloud_services
						.key_manager()
						.map(|res| res.map_err(Into::into)),
				)
					.try_join()
					.await?;

				super::handle_comm_error(
					client
						.sync()
						.groups()
						.leave(groups::leave::Request {
							access_token,
							pub_id,
							current_device_pub_id,
						})
						.await,
					"Failed to leave sync group;",
				)??;

				key_manager.remove_group(pub_id, &mut rng).await?;

				debug!(%pub_id, "Left sync group");

				Ok(())
			})
		})
		.procedure("list", {
			R.query(|node, _: ()| async move {
				use groups::list::{Request, Response};

				let (client, access_token) = super::get_client_and_access_token(&node).await?;

				let Response(groups) = super::handle_comm_error(
					client.sync().groups().list(Request { access_token }).await,
					"Failed to list groups;",
				)??;

				debug!(?groups, "Listed sync groups");

				Ok(groups)
			})
		})
		.procedure("remove_device", {
			#[derive(Deserialize, specta::Type)]
			struct CloudSyncGroupsRemoveDeviceArgs {
				group_pub_id: groups::PubId,
				to_remove_device_pub_id: devices::PubId,
			}
			R.query(
				|node,
				 CloudSyncGroupsRemoveDeviceArgs {
				     group_pub_id,
				     to_remove_device_pub_id,
				 }: CloudSyncGroupsRemoveDeviceArgs| async move {
					use groups::remove_device::Request;

					let ((client, access_token), current_device_pub_id, mut rng, key_manager) = (
						super::get_client_and_access_token(&node),
						node.config.get().map(|config| Ok(config.id.into())),
						node.master_rng
							.lock()
							.map(|mut rng| Ok(CryptoRng::from_seed(rng.generate_fixed()))),
						node.cloud_services
							.key_manager()
							.map(|res| res.map_err(Into::into)),
					)
						.try_join()
						.await?;

					let new_key = SecretKey::generate(&mut rng);
					let new_key_hash = KeyHash(blake3::hash(new_key.as_ref()).to_hex().to_string());

					key_manager
						.add_key_with_hash(group_pub_id, new_key, new_key_hash.clone(), &mut rng)
						.await?;

					super::handle_comm_error(
						client
							.sync()
							.groups()
							.remove_device(Request {
								access_token,
								group_pub_id,
								new_key_hash,
								current_device_pub_id,
								to_remove_device_pub_id,
							})
							.await,
						"Failed to remove device from sync group;",
					)??;

					debug!(%to_remove_device_pub_id, %group_pub_id, "Removed device");

					Ok(())
				},
			)
		})
		.procedure("request_join", {
			#[derive(Deserialize, specta::Type)]
			struct SyncGroupsRequestJoinArgs {
				sync_group: groups::GroupWithDevices,
				asking_device: devices::Device,
			}

			R.mutation(
				|node,
				 SyncGroupsRequestJoinArgs {
				     sync_group,
				     asking_device,
				 }: SyncGroupsRequestJoinArgs| async move {
					let ((client, access_token), current_device_pub_id, cloud_p2p) = (
						super::get_client_and_access_token(&node),
						node.config.get().map(|config| Ok(config.id.into())),
						node.cloud_services
							.cloud_p2p()
							.map(|res| res.map_err(Into::into)),
					)
						.try_join()
						.await?;

					let group_pub_id = sync_group.pub_id;

					debug!("My pub id: {:?}", current_device_pub_id);
					debug!("Asking device pub id: {:?}", asking_device.pub_id);
					if asking_device.pub_id != current_device_pub_id {
						return Err(rspc::Error::new(
							rspc::ErrorCode::BadRequest,
							String::from("Asking device must be the current device"),
						));
					}

					let groups::request_join::Response(existing_devices) =
						super::handle_comm_error(
							client
								.sync()
								.groups()
								.request_join(groups::request_join::Request {
									access_token,
									group_pub_id,
									current_device_pub_id,
								})
								.await,
							"Failed to update library;",
						)??;

					let (tx, rx) = oneshot::channel();

					cloud_p2p
						.request_join_sync_group(
							existing_devices,
							cloud_p2p::authorize_new_device_in_sync_group::Request {
								sync_group,
								asking_device,
							},
							tx,
						)
						.await;

					JoinedSyncGroupReceiver {
						node,
						group_pub_id,
						rx,
					}
					.dispatch();

					debug!(%group_pub_id, "Requested to join sync group");

					Ok(())
				},
			)
		})
}

struct JoinedSyncGroupReceiver {
	node: Arc<Node>,
	group_pub_id: groups::PubId,
	rx: oneshot::Receiver<JoinedLibraryCreateArgs>,
}

impl JoinedSyncGroupReceiver {
	fn dispatch(self) {
		spawn(async move {
			let Self {
				node,
				group_pub_id,
				rx,
			} = self;

			if let Ok(JoinedLibraryCreateArgs {
				pub_id: libraries::PubId(pub_id),
				name,
				description,
			}) = rx.await
			{
				let Ok(name) =
					LibraryName::new(name).map_err(|e| error!(?e, "Invalid library name"))
				else {
					return;
				};

				let Ok(library) = node
					.libraries
					.create_with_uuid(pub_id, name, description, true, None, &node)
					.await
					.map_err(|e| {
						error!(?e, "Failed to create library from sync group join response")
					})
				else {
					return;
				};

				if let Err(e) = library.init_cloud_sync(&node, group_pub_id).await {
					error!(?e, "Failed to initialize cloud sync for library");
				}
			}
		});
	}
}
