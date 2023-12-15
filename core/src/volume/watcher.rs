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

	use super::get_volumes;
	spawn(async move {
		let mut interval = interval(Duration::from_secs(1));
		let mut existing_volumes = get_volumes().await.into_iter().collect::<HashSet<_>>();

		loop {
			interval.tick().await;

			let current_volumes = get_volumes().await.into_iter().collect::<HashSet<_>>();

			if existing_volumes != current_volumes {
				existing_volumes = current_volumes;
				invalidate_query!(&library, "volumes.list");
			}
		}
	});
}
