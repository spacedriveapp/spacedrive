use crate::api::{utils::library, Ctx, R};

use sd_cloud_schema::{auth::AccessToken, devices, libraries};

use futures_concurrency::future::TryJoin;
use rspc::alpha::AlphaRouter;
use serde::Deserialize;
use tracing::debug;

use super::try_get_cloud_services_client;

pub fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("get", {
			R.query(|node, req: libraries::get::Request| async move {
				let libraries::get::Response(library) = super::handle_comm_error(
					try_get_cloud_services_client(&node)
						.await?
						.libraries()
						.get(req)
						.await,
					"Failed to get library;",
				)??;

				debug!(?library, "Got library");

				Ok(library)
			})
		})
		.procedure("list", {
			R.query(|node, req: libraries::list::Request| async move {
				let libraries::list::Response(libraries) = super::handle_comm_error(
					try_get_cloud_services_client(&node)
						.await?
						.libraries()
						.list(req)
						.await,
					"Failed to list libraries;",
				)??;

				debug!(?libraries, "Listed libraries");

				Ok(libraries)
			})
		})
		.procedure("create", {
			R.with2(library())
				.mutation(|(node, library), access_token: AccessToken| async move {
					let (client, name, device_pub_id) = (
						try_get_cloud_services_client(&node),
						async { Ok(library.config().await.name.to_string()) },
						async { Ok(devices::PubId(node.config.get().await.id.into())) },
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
				.mutation(|(node, library), access_token: AccessToken| async move {
					super::handle_comm_error(
						try_get_cloud_services_client(&node)
							.await?
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
			#[derive(Deserialize, specta::Type)]
			struct LibrariesUpdateArgs {
				access_token: AccessToken,
				name: String,
			}

			R.with2(library()).mutation(
				|(node, library),
				 LibrariesUpdateArgs { access_token, name }: LibrariesUpdateArgs| async move {
					super::handle_comm_error(
						try_get_cloud_services_client(&node)
							.await?
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
				},
			)
		})
}
