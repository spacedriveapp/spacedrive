use crate::api::{Ctx, R};

use sd_cloud_schema::{devices, libraries, locations};

use rspc::alpha::AlphaRouter;
use serde::Deserialize;
use tracing::debug;

pub fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("list", {
			#[derive(Deserialize, specta::Type)]
			struct CloudListLocationsArgs {
				pub library_pub_id: libraries::PubId,
				pub with_library: bool,
				pub with_device: bool,
			}

			R.query(
				|node,
				 CloudListLocationsArgs {
				     library_pub_id,
				     with_library,
				     with_device,
				 }: CloudListLocationsArgs| async move {
					use locations::list::{Request, Response};

					let (client, access_token) = super::get_client_and_access_token(&node).await?;

					let Response(locations) = super::handle_comm_error(
						client
							.locations()
							.list(Request {
								access_token,
								library_pub_id,
								with_library,
								with_device,
							})
							.await,
						"Failed to list locations;",
					)??;

					debug!(?locations, "Got locations");

					Ok(locations)
				},
			)
		})
		.procedure("create", {
			#[derive(Deserialize, specta::Type)]
			struct CloudCreateLocationArgs {
				pub pub_id: locations::PubId,
				pub name: String,
				pub library_pub_id: libraries::PubId,
				pub device_pub_id: devices::PubId,
			}

			R.mutation(
				|node,
				 CloudCreateLocationArgs {
				     pub_id,
				     name,
				     library_pub_id,
				     device_pub_id,
				 }: CloudCreateLocationArgs| async move {
					use locations::create::Request;

					let (client, access_token) = super::get_client_and_access_token(&node).await?;

					super::handle_comm_error(
						client
							.locations()
							.create(Request {
								access_token,
								pub_id,
								name,
								library_pub_id,
								device_pub_id,
							})
							.await,
						"Failed to list locations;",
					)??;

					debug!("Created cloud location");

					Ok(())
				},
			)
		})
		.procedure("delete", {
			R.mutation(|node, pub_id: locations::PubId| async move {
				use locations::delete::Request;

				let (client, access_token) = super::get_client_and_access_token(&node).await?;

				super::handle_comm_error(
					client
						.locations()
						.delete(Request {
							access_token,
							pub_id,
						})
						.await,
					"Failed to list locations;",
				)??;

				debug!("Created cloud location");

				Ok(())
			})
		})
}
