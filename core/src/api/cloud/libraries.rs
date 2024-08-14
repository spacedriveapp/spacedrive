use crate::api::{utils::library, Ctx, R};

use sd_cloud_schema::{auth::AccessToken, devices, libraries};

use rspc::alpha::AlphaRouter;
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
			#[derive(Debug, serde::Serialize, serde::Deserialize, specta::Type)]
			struct LibrariesCreateArgs {
				access_token: AccessToken,
				device_pub_id: devices::PubId,
			}

			R.with2(library())
				.mutation(|(node, library), args: LibrariesCreateArgs| async move {
					let req = libraries::create::Request {
						name: library.config().await.name.to_string(),
						access_token: args.access_token,
						pub_id: libraries::PubId(library.id),
						device_pub_id: args.device_pub_id,
					};
					super::handle_comm_error(
						try_get_cloud_services_client(&node)
							.await?
							.libraries()
							.create(req)
							.await,
						"Failed to create library;",
					)??;

					Ok(())
				})
		})
		.procedure("delete", {
			R.mutation(|node, req: libraries::delete::Request| async move {
				super::handle_comm_error(
					try_get_cloud_services_client(&node)
						.await?
						.libraries()
						.delete(req)
						.await,
					"Failed to delete library;",
				)??;

				debug!("Deleted library");

				Ok(())
			})
		})
		.procedure("update", {
			R.mutation(|node, req: libraries::update::Request| async move {
				super::handle_comm_error(
					try_get_cloud_services_client(&node)
						.await?
						.libraries()
						.update(req)
						.await,
					"Failed to update library;",
				)??;

				debug!("Updated library");

				Ok(())
			})
		})
}
