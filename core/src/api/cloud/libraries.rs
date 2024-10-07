use crate::api::{utils::library, Ctx, R};

use sd_cloud_schema::libraries;

use futures::FutureExt;
use futures_concurrency::future::TryJoin;
use rspc::alpha::AlphaRouter;
use serde::Deserialize;
use tracing::debug;

pub fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("get", {
			#[derive(Deserialize, specta::Type)]
			struct CloudGetLibraryArgs {
				pub_id: libraries::PubId,
				with_device: bool,
			}

			R.query(
				|node,
				 CloudGetLibraryArgs {
				     pub_id,
				     with_device,
				 }: CloudGetLibraryArgs| async move {
					use libraries::get::{Request, Response};

					let (client, access_token) = super::get_client_and_access_token(&node).await?;

					let Response(library) = super::handle_comm_error(
						client
							.libraries()
							.get(Request {
								access_token,
								pub_id,
								with_device,
							})
							.await,
						"Failed to get library;",
					)??;

					debug!(?library, "Got library");

					Ok(library)
				},
			)
		})
		.procedure("list", {
			R.query(|node, with_device: bool| async move {
				use libraries::list::{Request, Response};

				let (client, access_token) = super::get_client_and_access_token(&node).await?;

				let Response(libraries) = super::handle_comm_error(
					client
						.libraries()
						.list(Request {
							access_token,
							with_device,
						})
						.await,
					"Failed to list libraries;",
				)??;

				debug!(?libraries, "Listed libraries");

				Ok(libraries)
			})
		})
		.procedure("create", {
			R.with2(library())
				.mutation(|(node, library), _: ()| async move {
					let ((client, access_token), name, device_pub_id) = (
						super::get_client_and_access_token(&node),
						library.config().map(|config| Ok(config.name.to_string())),
						node.config.get().map(|config| Ok(config.id.into())),
					)
						.try_join()
						.await?;

					super::handle_comm_error(
						client
							.libraries()
							.create(libraries::create::Request {
								name,
								access_token,
								pub_id: libraries::PubId(library.id),
								device_pub_id,
							})
							.await,
						"Failed to create library;",
					)??;

					Ok(())
				})
		})
		.procedure("delete", {
			R.with2(library())
				.mutation(|(node, library), _: ()| async move {
					let (client, access_token) = super::get_client_and_access_token(&node).await?;

					super::handle_comm_error(
						client
							.libraries()
							.delete(libraries::delete::Request {
								access_token,
								pub_id: libraries::PubId(library.id),
							})
							.await,
						"Failed to delete library;",
					)??;

					debug!("Deleted library");

					Ok(())
				})
		})
		.procedure("update", {
			R.with2(library())
				.mutation(|(node, library), name: String| async move {
					let (client, access_token) = super::get_client_and_access_token(&node).await?;

					super::handle_comm_error(
						client
							.libraries()
							.update(libraries::update::Request {
								access_token,
								pub_id: libraries::PubId(library.id),
								name,
							})
							.await,
						"Failed to update library;",
					)??;

					debug!("Updated library");

					Ok(())
				})
		})
}
