use crate::{node::config::NodeConfig, volume::get_volumes, Node};

use sd_core_cloud_services::{CloudP2P, IrohSecretKey, KeyManager, QuinnConnection, UserResponse};

use sd_cloud_schema::{
	auth,
	error::{ClientSideError, Error},
	users, Client, Service,
};
use sd_crypto::{CryptoRng, SeedableRng};

use std::pin::pin;

use async_stream::stream;
use futures::StreamExt;
use rspc::alpha::AlphaRouter;
use tracing::error;

use super::{Ctx, R};

mod devices;
mod libraries;
mod locations;

async fn try_get_cloud_services_client(
	node: &Node,
) -> Result<Client<QuinnConnection<Service>, Service>, sd_core_cloud_services::Error> {
	node.cloud_services
		.client()
		.await
		.map_err(::sd_utils::error::report_error(
			"Failed to get cloud services client;",
		))
}

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.merge("library.", library::mount())
		.merge("libraries.", libraries::mount())
		.merge("locations.", locations::mount())
		.merge("devices.", devices::mount())
		.procedure("bootstrap", {
			R.mutation(
				|node, (access_token, refresh_token): (auth::AccessToken, auth::RefreshToken)| async move {
					use sd_cloud_schema::devices;

					node.cloud_services
						.token_refresher
						.init(access_token.clone(), refresh_token)
						.await?;

					let client = try_get_cloud_services_client(&node).await?;
					let data_directory = node.config.data_directory();

					let mut rng =
						CryptoRng::from_seed(node.master_rng.lock().await.generate_fixed());

					// create user route is idempotent, so we can safely keep creating the same user over and over
					handle_comm_error(
						client
							.users()
							.create(users::create::Request {
								access_token: access_token.clone(),
							})
							.await,
						"Failed to create user;",
					)??;

					let (device_pub_id, name, os) = {
						let NodeConfig { id, name, os, .. } = node.config.get().await;
						(devices::PubId(id.into()), name, os)
					};
					let mut hasher = blake3::Hasher::new();
					hasher.update(device_pub_id.0.as_bytes().as_slice());
					let hashed_pub_id = hasher.finalize();

					let key_manager = match handle_comm_error(
						client
							.devices()
							.get(devices::get::Request {
								access_token: access_token.clone(),
								pub_id: device_pub_id,
							})
							.await,
						"Failed to get device on cloud bootstrap;",
					)? {
						Ok(_) => {
							// Device registered, we execute a device hello flow
							let master_key = self::devices::hello(
								&client,
								access_token,
								device_pub_id,
								hashed_pub_id,
								&mut rng,
							)
							.await?;

							KeyManager::load(master_key, data_directory).await?
						}
						Err(Error::Client(ClientSideError::NotFound(_))) => {
							// Device not registered, we execute a device register flow
							let iroh_secret_key = IrohSecretKey::generate_with_rng(&mut rng);

							let master_key = self::devices::register(
								&client,
								access_token,
								self::devices::DeviceRegisterData {
									pub_id: device_pub_id,
									name,
									os,
									// TODO(@fogodev): We should use storage statistics from sqlite db
									storage_size: get_volumes()
										.await
										.into_iter()
										.map(|volume| volume.total_capacity)
										.sum(),
									connection_id: iroh_secret_key.public(),
								},
								hashed_pub_id,
								&mut rng,
							)
							.await?;

							KeyManager::new(master_key, iroh_secret_key, data_directory, &mut rng)
								.await?
						}
						Err(e) => return Err(e.into()),
					};

					let iroh_secret_key = key_manager.iroh_secret_key().await;

					node.cloud_services.set_key_manager(key_manager).await;

					node.cloud_services
						.set_cloud_p2p(
							CloudP2P::new(
								device_pub_id,
								&node.cloud_services,
								rng,
								iroh_secret_key,
								node.cloud_services.cloud_p2p_dns_origin_name.clone(),
								node.cloud_services.cloud_p2p_relay_url.clone(),
							)
							.await?,
						)
						.await;

					Ok(())
				},
			)
		})
		.procedure(
			"listenCloudServicesNotifications",
			R.subscription(|node, _: ()| async move {
				stream! {
					let mut notifications_stream =
					pin!(node.cloud_services.stream_user_notifications());

					while let Some(notification) = notifications_stream.next().await {
						yield notification;
					}
				}
			}),
		)
		.procedure(
			"userResponse",
			R.mutation(|node, response: UserResponse| async move {
				node.cloud_services.send_user_response(response).await;

				Ok(())
			}),
		)
}

fn handle_comm_error<T, E: std::error::Error + std::fmt::Debug + Send + Sync + 'static>(
	res: Result<T, E>,
	message: &'static str,
) -> Result<T, rspc::Error> {
	res.map_err(|e| {
		error!(?e, "Communication with cloud services error: {message}");
		rspc::Error::with_cause(rspc::ErrorCode::InternalServerError, message.into(), e)
	})
}
