use crate::api::{utils::library, Ctx, R};

use sd_cloud_schema::{
	auth::AccessToken,
	cloud_p2p, devices, libraries,
	sync::{groups, KeyHash},
};

use futures_concurrency::future::TryJoin;
use rspc::alpha::AlphaRouter;
use sd_crypto::{cloud::secret_key::SecretKey, CryptoRng, SeedableRng};
use serde::Deserialize;
use tracing::debug;

pub fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("create", {
			R.with2(library())
				.mutation(|(node, library), access_token: AccessToken| async move {
					let (client, device_pub_id, mut rng, key_manager) = (
						super::try_get_cloud_services_client(&node),
						async { Ok(devices::PubId(node.config.get().await.id.into())) },
						async {
							Ok(CryptoRng::from_seed(
								node.master_rng.lock().await.generate_fixed(),
							))
						},
						node.cloud_services.key_manager(),
					)
						.try_join()
						.await?;

					let new_key = SecretKey::generate(&mut rng);
					let key_hash = KeyHash(blake3::hash(new_key.as_ref()).to_hex().to_string());

					let groups::create::Response(group_pub_id) = super::handle_comm_error(
						client
							.sync()
							.groups()
							.create(groups::create::Request {
								access_token: access_token.clone(),
								key_hash: key_hash.clone(),
								library_pub_id: libraries::PubId(library.id),
								device_pub_id,
							})
							.await,
						"Failed to create sync group;",
					)??;

					if let Err(e) = key_manager
						.add_key_with_hash(group_pub_id, new_key, key_hash, &mut rng)
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

					// TODO(@fogodev): use the group_pub_id to dispatch actors for syncing to this group

					debug!(%group_pub_id, "Created sync group");

					Ok(())
				})
		})
		.procedure("delete", {
			R.mutation(|node, req: groups::delete::Request| async move {
				let group_pub_id = req.pub_id;
				super::handle_comm_error(
					super::try_get_cloud_services_client(&node)
						.await?
						.sync()
						.groups()
						.delete(req)
						.await,
					"Failed to delete sync group;",
				)??;

				debug!(%group_pub_id, "Deleted sync group");

				Ok(())
			})
		})
		.procedure("get", {
			R.query(|node, req: groups::get::Request| async move {
				let groups::get::Response(group) = super::handle_comm_error(
					super::try_get_cloud_services_client(&node)
						.await?
						.sync()
						.groups()
						.get(req)
						.await,
					"Failed to get sync group;",
				)??;

				debug!(?group, "Got sync group");

				Ok(group)
			})
		})
		.procedure("leave", {
			#[derive(Deserialize, specta::Type)]
			struct SyncGroupsLeaveArgs {
				access_token: AccessToken,
				group_pub_id: groups::PubId,
			}

			R.query(
				|node,
				 SyncGroupsLeaveArgs {
				     access_token,
				     group_pub_id,
				 }: SyncGroupsLeaveArgs| async move {
					let (device_pub_id, client, key_manager) = (
						async { Ok(node.config.get().await.id) },
						super::try_get_cloud_services_client(&node),
						node.cloud_services.key_manager(),
					)
						.try_join()
						.await?;

					super::handle_comm_error(
						client
							.sync()
							.groups()
							.leave(groups::leave::Request {
								access_token,
								pub_id: group_pub_id,
								current_device_pub_id: devices::PubId(device_pub_id.into()),
							})
							.await,
						"Failed to leave sync group;",
					)??;

					let mut rng =
						CryptoRng::from_seed(node.master_rng.lock().await.generate_fixed());

					key_manager.remove_group(group_pub_id, &mut rng).await?;

					debug!(%group_pub_id, "Left sync group");

					Ok(())
				},
			)
		})
		.procedure("list", {
			R.query(|node, req: groups::list::Request| async move {
				let groups::list::Response(groups) = super::handle_comm_error(
					super::try_get_cloud_services_client(&node)
						.await?
						.sync()
						.groups()
						.list(req)
						.await,
					"Failed to list groups;",
				)??;

				debug!(?groups, "Listed sync groups");

				Ok(groups)
			})
		})
		.procedure("remove_device", {
			#[derive(Deserialize, specta::Type)]
			struct SyncGroupsRemoveDeviceArgs {
				access_token: AccessToken,
				group_pub_id: groups::PubId,
				to_remove_device_pub_id: devices::PubId,
			}
			R.query(
				|node,
				 SyncGroupsRemoveDeviceArgs {
				     access_token,
				     group_pub_id,
				     to_remove_device_pub_id,
				 }: SyncGroupsRemoveDeviceArgs| async move {
					let (client, current_device_pub_id, mut rng, key_manager) = (
						super::try_get_cloud_services_client(&node),
						async { Ok(devices::PubId(node.config.get().await.id.into())) },
						async {
							Ok(CryptoRng::from_seed(
								node.master_rng.lock().await.generate_fixed(),
							))
						},
						node.cloud_services.key_manager(),
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
							.remove_device(groups::remove_device::Request {
								access_token,
								group_pub_id,
								new_key_hash,
								current_device_pub_id,
								to_remove_device_pub_id,
							})
							.await,
						"Failed to list libraries;",
					)??;

					debug!(%to_remove_device_pub_id, %group_pub_id, "Removed device");

					Ok(())
				},
			)
		})
		.procedure("request_join", {
			#[derive(Deserialize, specta::Type)]
			struct SyncGroupsRequestJoinArgs {
				access_token: AccessToken,
				sync_group: groups::GroupWithLibraryAndDevices,
				asking_device: devices::Device,
			}

			R.mutation(
				|node,
				 SyncGroupsRequestJoinArgs {
				     access_token,
				     sync_group,
				     asking_device,
				 }: SyncGroupsRequestJoinArgs| async move {
					let (client, current_device_pub_id, cloud_p2p) = (
						super::try_get_cloud_services_client(&node),
						async { Ok(devices::PubId(node.config.get().await.id.into())) },
						node.cloud_services.cloud_p2p(),
					)
						.try_join()
						.await?;

					let group_pub_id = sync_group.pub_id;

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

					cloud_p2p
						.request_join_sync_group(
							existing_devices,
							cloud_p2p::authorize_new_device_in_sync_group::Request {
								sync_group,
								asking_device,
							},
						)
						.await;

					debug!(%group_pub_id, "Requested to join sync group");

					Ok(())
				},
			)
		})
}
