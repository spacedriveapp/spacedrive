// This file is being deprecated in favor of libraries.rs
// This is due to the migration to the new API system, but the frontend is still using this file

use crate::api::{Ctx, R};

use rspc::alpha::AlphaRouter;

pub fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("list", {
			R.query(|node, _: ()| async move {
				sd_cloud_api::locations::list(node.cloud_api_config().await)
					.await
					.map_err(Into::into)
			})
		})
		.procedure("create", {
			R.mutation(|node, name: String| async move {
				sd_cloud_api::locations::create(node.cloud_api_config().await, name)
					.await
					.map_err(Into::into)
			})
		})
		.procedure("remove", {
			R.mutation(|node, id: String| async move {
				sd_cloud_api::locations::create(node.cloud_api_config().await, id)
					.await
					.map_err(Into::into)
			})
		})
}
