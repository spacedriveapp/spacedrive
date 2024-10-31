#[cfg(not(target_os = "linux"))]
use crate::{invalidate_query, library::Library};

#[cfg(not(target_os = "linux"))]
use std::{collections::HashSet, sync::Arc};

#[cfg(not(target_os = "linux"))]
pub fn spawn_volume_watcher(library: Arc<Library>) {
	use tokio::{
		spawn,
		time::{interval, Duration},
	};
	use tracing::error;

	use super::get_volumes;
	spawn(async move {
		let mut interval = interval(Duration::from_secs(1));
		let mut existing_volumes = get_volumes().await.into_iter().collect::<HashSet<_>>();

		loop {
			interval.tick().await;

			let current_volumes = get_volumes().await.into_iter().collect::<HashSet<_>>();

			if existing_volumes != current_volumes {
				existing_volumes = current_volumes;
				let (total_capacity, available_capacity) = super::compute_stats(&existing_volumes);

				if let Err(e) = super::update_storage_statistics(
					&library.db,
					&library.sync,
					total_capacity,
					available_capacity,
				)
				.await
				{
					error!(?e, "Failed to update storage statistics;");
				}

				invalidate_query!(&library, "volumes.list");
			}
		}
	});
}
