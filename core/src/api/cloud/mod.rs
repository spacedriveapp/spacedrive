use crate::{
	invalidate_query,
	library::LibraryManagerError,
	node::{config::NodeConfig, HardwareModel},
	Node,
};

use sd_core_cloud_services::{CloudP2P, KeyManager, QuinnConnection, UserResponse};

use sd_cloud_schema::{
	auth,
	error::{ClientSideError, Error},
	sync::groups,
	users, Client, Request, Response, SecretKey as IrohSecretKey,
};
use sd_crypto::{CryptoRng, SeedableRng};
use sd_utils::error::report_error;

use std::pin::pin;

use async_stream::stream;
use futures::{FutureExt, StreamExt};
use futures_concurrency::future::TryJoin;
use rspc::alpha::AlphaRouter;
use tracing::{debug, error, instrument};

use super::{Ctx, R};

mod devices;
mod libraries;
mod locations;
mod sync_groups;

async fn try_get_cloud_services_client(
	node: &Node,
) -> Result<Client<QuinnConnection<Response, Request>>, sd_core_cloud_services::Error> {
	node.cloud_services
		.client()
		.await
		.map_err(report_error("Failed to get cloud services client"))
}

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.merge("libraries.", libraries::mount())
		.merge("locations.", locations::mount())
		.merge("devices.", devices::mount())
		.merge("syncGroups.", sync_groups::mount())
		.procedure("bootstrap", {
			R.mutation(
				|node, (access_token, refresh_token): (auth::AccessToken, auth::RefreshToken)| async move {
					use sd_cloud_schema::devices;

					// Only allow a single bootstrap request in flight at a time
					let mut has_bootstrapped_lock = node
						.cloud_services
						.has_bootstrapped
						.try_lock()
						.map_err(|_| {
							rspc::Error::new(
								rspc::ErrorCode::Conflict,
								String::from("Bootstrap in progress"),
							)
						})?;

					if *has_bootstrapped_lock {
						return Err(rspc::Error::new(
							rspc::ErrorCode::Conflict,
							String::from("Already bootstrapped"),
						));
					}

					node.cloud_services
						.token_refresher
						.init(access_token, refresh_token)
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
								access_token: node
									.cloud_services
									.token_refresher
									.get_access_token()
									.await?,
							})
							.await,
						"Failed to create user;",
					)??;

					let (device_pub_id, name, os) = {
						let NodeConfig { id, name, os, .. } = node.config.get().await;
						(devices::PubId(id.into()), name, os)
					};

					let hashed_pub_id = blake3::hash(device_pub_id.0.as_bytes().as_slice());

					let key_manager = match handle_comm_error(
						client
							.devices()
							.get(devices::get::Request {
								access_token: node
									.cloud_services
									.token_refresher
									.get_access_token()
									.await?,
								pub_id: device_pub_id,
							})
							.await,
						"Failed to get device on cloud bootstrap;",
					)? {
						Ok(_) => {
							// Device registered, we execute a device hello flow
							let master_key = self::devices::hello(
								&client,
								node.cloud_services
									.token_refresher
									.get_access_token()
									.await?,
								device_pub_id,
								hashed_pub_id,
								&mut rng,
							)
							.await?;

							debug!("Device hello successful");

							KeyManager::load(master_key, data_directory).await?
						}
						Err(Error::Client(ClientSideError::NotFound(_))) => {
							// Device not registered, we execute a device register flow
							let iroh_secret_key = IrohSecretKey::generate_with_rng(&mut rng);
							let hardware_model = Into::into(
								HardwareModel::try_get().unwrap_or(HardwareModel::Other),
							);

							let master_key = self::devices::register(
								&client,
								node.cloud_services
									.token_refresher
									.get_access_token()
									.await?,
								self::devices::DeviceRegisterData {
									pub_id: device_pub_id,
									name,
									os,
									hardware_model,
									connection_id: iroh_secret_key.public(),
								},
								hashed_pub_id,
								&mut rng,
							)
							.await?;

							debug!("Device registered successfully");

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
								node.cloud_services.cloud_p2p_dns_pkarr_url.clone(),
								node.cloud_services.cloud_p2p_relay_url.clone(),
							)
							.await?,
						)
						.await;

					let groups::list::Response(groups) = handle_comm_error(
						client
							.sync()
							.groups()
							.list(groups::list::Request {
								access_token: node
									.cloud_services
									.token_refresher
									.get_access_token()
									.await?,
							})
							.await,
						"Failed to list sync groups on bootstrap",
					)??;

					groups
						.into_iter()
						.map(
							|groups::GroupBaseData {
							     pub_id,
							     library,
							     // TODO(@fogodev): We can use this latest key hash to check if we
							     // already have the latest key hash for this group locally
							     // issuing a ask for key hash request for other devices if we don't
							     latest_key_hash: _latest_key_hash,
							     ..
							 }| {
								let node = &node;

								async move {
									match initialize_cloud_sync(pub_id, library, node).await {
										// If we don't have this library locally, we didn't joined this group yet
										Ok(()) | Err(LibraryManagerError::LibraryNotFound) => {
											Ok(())
										}
										Err(e) => Err(e),
									}
								}
							},
						)
						.collect::<Vec<_>>()
						.try_join()
						.await?;

					*has_bootstrapped_lock = true;

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
		.procedure(
			"hasBootstrapped",
			R.query(|node, _: ()| async move {
				// If we can't lock immediately, it means that there is a bootstrap in progress
				// so we didn't bootstrapped yet
				Ok(node
					.cloud_services
					.has_bootstrapped
					.try_lock()
					.map(|lock| *lock)
					.unwrap_or(false))
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

#[instrument(skip_all, fields(%group_pub_id, %library_pub_id), err)]
async fn initialize_cloud_sync(
	group_pub_id: groups::PubId,
	sd_cloud_schema::libraries::Library {
		pub_id: sd_cloud_schema::libraries::PubId(library_pub_id),
		..
	}: sd_cloud_schema::libraries::Library,
	node: &Node,
) -> Result<(), LibraryManagerError> {
	let library = node
		.libraries
		.get_library(&library_pub_id)
		.await
		.ok_or(LibraryManagerError::LibraryNotFound)?;

	library.init_cloud_sync(node, group_pub_id).await
}

async fn get_client_and_access_token(
	node: &Node,
) -> Result<
	(
		Client<QuinnConnection<Response, Request>>,
		auth::AccessToken,
	),
	rspc::Error,
> {
	(
		try_get_cloud_services_client(node),
		node.cloud_services
			.token_refresher
			.get_access_token()
			.map(|res| res.map_err(Into::into)),
	)
		.try_join()
		.await
		.map_err(Into::into)
}
