use super::{utils::library, Ctx, R};
use crate::library::Library;
use rspc::alpha::AlphaRouter;
use serde::Deserialize;
use specta::Type;

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router().procedure(
		"list",
		R.query(|node, _: ()| async move {
			tracing::debug!("Handling volumes list request");
			// Add a map_err to properly convert the error
			match node.volumes.list_system_volumes().await {
				Ok(volumes) => {
					tracing::debug!("Returning {} volumes", volumes.len());
					Ok(volumes)
				}
				Err(e) => {
					tracing::error!("Error listing volumes: {:?}", e);
					Err(e.into())
				}
			}
		}),
	)
}
