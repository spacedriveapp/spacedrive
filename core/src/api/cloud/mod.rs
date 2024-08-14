use crate::Node;

use sd_cloud_schema::{
	auth,
	error::{ClientSideError, Error},
	users, Client, Service,
};
use sd_core_cloud_services::QuinnConnection;

use rspc::alpha::AlphaRouter;
use tracing::error;
use uuid::Uuid;

use super::{Ctx, R};

mod devices;
mod libraries;
mod library;
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
	// .procedure("bootstrap", {
	// 	R.mutation(|node, access_token: auth::AccessToken| async move {
	// 		use sd_cloud_schema::devices;

	// 		let client = try_get_cloud_services_client(&node).await?;

	// 		// create user route is idempotent, so we can safely keep creating the same user over and over
	// 		handle_comm_error(
	// 			client
	// 				.users()
	// 				.create(users::create::Request {
	// 					access_token: access_token.clone(),
	// 				})
	// 				.await,
	// 			"Failed to create user;",
	// 		)??;

	// 		let device_pub_id = devices::PubId(node.config.get().await.id);
	// 		let mut hasher = blake3::Hasher::new();
	// 		hasher.update(device_pub_id.0.as_bytes().as_slice());
	// 		let hashed_pub_id = hasher.finalize();

	// 		match handle_comm_error(
	// 			client
	// 				.devices()
	// 				.get(devices::get::Request {
	// 					access_token: access_token.clone(),
	// 					pub_id: device_pub_id,
	// 				})
	// 				.await,
	// 			"Failed to get device on cloud bootstrap;",
	// 		)? {
	// 			Ok(_) => {
	// 				// Device registered, we execute a device hello flow
	// 				self::devices::hello(&client, access_token, device_pub_id, hashed_pub_id)
	// 					.await
	// 			}
	// 			Err(Error::Client(ClientSideError::NotFound(_))) => {
	// 				// Device not registered, we execute a device register flow
	// 				todo!()
	// 			}
	// 			Err(e) => return Err(e.into()),
	// 		}

	// 		// TODO: figure out a way to know if we need to register the device or send a device hello request

	// 		// TODO: in case of a device register request, we use the OPAQUE key to encrypt iroh's secret key (NodeId)
	// 		// and save on data directory

	// 		// TODO: in case of a device hello request, we use the OPAQUE key to decrypt iroh's secret key (NodeId)
	// 		// and keep it in memory

	// 		// TODO: With this device iroh's secret key (NodeId) now known and we can start the iroh
	// 		// node for cloud p2p

	// 		Ok(())
	// 	})
	// })
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
