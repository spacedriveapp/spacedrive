use crate::{
	api::{Ctx, R},
	try_get_cloud_services_client,
};

use rspc::alpha::AlphaRouter;
use sd_cloud_schema::locations;
use tracing::debug;

pub fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("get", {
			R.query(|node, req: locations::get::Request| async move {
				let locations::get::Response(location) = super::handle_comm_error(
					try_get_cloud_services_client!(node)?
						.locations()
						.get(req)
						.await,
					"Failed to get location;",
				)??;

				debug!(?location, "Got location");

				Ok(location)
			})
		})
		.procedure("list", {
			R.query(|node, req: locations::list::Request| async move {
				let locations::list::Response(locations) = super::handle_comm_error(
					try_get_cloud_services_client!(node)?
						.locations()
						.list(req)
						.await,
					"Failed to list locations;",
				)??;

				debug!(?locations, "Listed locations");

				Ok(locations)
			})
		})
		.procedure("create", {
			R.mutation(|node, req: locations::create::Request| async move {
				super::handle_comm_error(
					try_get_cloud_services_client!(node)?
						.locations()
						.create(req)
						.await,
					"Failed to create location;",
				)??;

				debug!("Created location");

				// Should we invalidate the location list cache here?

				Ok(())
			})
		})
		.procedure("delete", {
			R.mutation(|node, req: locations::delete::Request| async move {
				super::handle_comm_error(
					try_get_cloud_services_client!(node)?
						.locations()
						.delete(req)
						.await,
					"Failed to delete location;",
				)??;

				debug!("Deleted location");

				Ok(())
			})
		})
		.procedure("update", {
			R.mutation(|node, req: locations::update::Request| async move {
				super::handle_comm_error(
					try_get_cloud_services_client!(node)?
						.locations()
						.update(req)
						.await,
					"Failed to update location;",
				)??;

				debug!("Updated location");

				Ok(())
			})
		})
}
