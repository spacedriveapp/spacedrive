use crate::{
	api::{Ctx, R},
	try_get_cloud_services_client,
};

use sd_cloud_schema::devices;

use rspc::alpha::AlphaRouter;
use tracing::debug;

pub fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("get", {
			R.query(|node, req: devices::get::Request| async move {
				let devices::get::Response(device) = super::handle_comm_error(
					try_get_cloud_services_client!(node)?
						.devices()
						.get(req)
						.await,
					"Failed to get device;",
				)??;

				debug!(?device, "Got device");

				Ok(device)
			})
		})
		.procedure("list", {
			R.query(|node, req: devices::list::Request| async move {
				let devices::list::Response(devices) = super::handle_comm_error(
					try_get_cloud_services_client!(node)?
						.devices()
						.list(req)
						.await,
					"Failed to list devices;",
				)??;

				debug!(?devices, "Listed devices");

				Ok(devices)
			})
		})
		.procedure("delete", {
			R.mutation(|node, req: devices::delete::Request| async move {
				super::handle_comm_error(
					try_get_cloud_services_client!(node)?
						.devices()
						.delete(req)
						.await,
					"Failed to delete device;",
				)??;

				debug!("Deleted device");

				Ok(())
			})
		})
		.procedure("update", {
			R.mutation(|node, req: devices::update::Request| async move {
				super::handle_comm_error(
					try_get_cloud_services_client!(node)?
						.devices()
						.update(req)
						.await,
					"Failed to update device;",
				)??;

				debug!("Updated device");

				Ok(())
			})
		})
}
