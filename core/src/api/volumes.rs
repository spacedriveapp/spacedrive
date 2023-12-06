use rspc::alpha::AlphaRouter;
use sd_cache::{Normalise, NormalisedResults};

use crate::volume::get_volumes;

use super::{Ctx, R};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router().procedure("list", {
		R.query(|_, _: ()| async move {
			let volumes = get_volumes().await;

			let (nodes, items) = volumes.normalise(|i| {
				// TODO: This is a really bad key. Once we hook up volumes with the DB fix this!
				blake3::hash(
					&i.mount_points
						.iter()
						.map(|mp| mp.as_os_str().to_string_lossy().as_bytes().to_vec())
						.flatten()
						.collect::<Vec<u8>>(),
				)
				.to_hex()
				.to_string()
			});

			Ok(NormalisedResults { nodes, items })
		})
	})
}
