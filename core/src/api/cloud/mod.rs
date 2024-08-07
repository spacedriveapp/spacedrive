use crate::{api::libraries::LibraryConfigWrapped, invalidate_query, library::LibraryName};

use sd_cloud_schema::{auth, users};

use rspc::alpha::AlphaRouter;
use tracing::error;
use uuid::Uuid;

use super::{Ctx, R};

mod devices;
mod library;
mod locations;

#[macro_export]
macro_rules! try_get_cloud_services_client {
	($node:expr) => {{
		let node: &$crate::Node = &$node;

		node.cloud_services
			.client()
			.await
			.map_err(::sd_utils::error::report_error(
				"Failed to get cloud services client;",
			))
	}};
}

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.merge("library.", library::mount())
		.merge("locations.", locations::mount())
		.merge("devices.", devices::mount())
		.procedure("bootstrap", {
			R.mutation(|node, access_token: auth::AccessToken| async move {
				let client = try_get_cloud_services_client!(node)?;

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

				// TODO: figure out a way to know if we need to register the device or send a device hello request

				// TODO: in case of a device register request, we use the OPAQUE key to encrypt iroh's secret key (NodeId)
				// and save on data directory

				// TODO: in case of a device hello request, we use the OPAQUE key to decrypt iroh's secret key (NodeId)
				// and keep it in memory

				// TODO: With this device iroh's secret key (NodeId) now known and we can start the iroh
				// node for cloud p2p

				Ok(())
			})
		})
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
