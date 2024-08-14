use crate::api::{Ctx, R};

use sd_cloud_schema::locations;

use rspc::alpha::AlphaRouter;
use tracing::debug;

pub fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("list", {
			R.query(|node, req: locations::list::Request| async move {
				let locations::list::Response(locations) = super::handle_comm_error(
					super::try_get_cloud_services_client(&node)
						.await?
						.locations()
						.list(req)
						.await,
					"Failed to list locations;",
				)??;

				debug!(?locations, "Got locations");

				Ok(locations)
			})
		})
		.procedure("create", {
			R.mutation(|node, req: locations::create::Request| async move {
				super::handle_comm_error(
					super::try_get_cloud_services_client(&node)
						.await?
						.locations()
						.create(req)
						.await,
					"Failed to list locations;",
				)??;

				debug!("Created cloud location");

				Ok(())
			})
		})
		.procedure("delete", {
			R.mutation(|node, req: locations::delete::Request| async move {
				super::handle_comm_error(
					super::try_get_cloud_services_client(&node)
						.await?
						.locations()
						.delete(req)
						.await,
					"Failed to list locations;",
				)??;

				debug!("Created cloud location");

				Ok(())
			})
		})
}
