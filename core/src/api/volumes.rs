use super::{utils::library, Ctx, R};
use crate::volume::{VolumeEvent, VolumeFingerprint};
use rspc::alpha::AlphaRouter;
use serde::Deserialize;
use specta::Type;

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure(
			"list",
			R.with2(library())
				.query(|(node, library), _: ()| async move {
					match node.volumes.list_system_volumes(library).await {
						Ok(volumes) => Ok(volumes),
						Err(e) => {
							tracing::error!("Error listing volumes: {:?}", e);
							Err(e.into())
						}
					}
				}),
		)
		.procedure(
			"track",
			R.with2(library()).mutation(
				|(node, library), fingerprint: VolumeFingerprint| async move {
					tracing::debug!(
						"Handling track volume request for volume_id={:?}",
						fingerprint
					);

					node.volumes
						.track_volume(fingerprint, library)
						.await
						.map_err(|e| {
							tracing::error!("Failed to track volume: {:?}", e);
							e.into()
						})
				},
			),
		)
		.procedure(
			"listForLibrary",
			R.with2(library())
				.query(|(node, library), _: ()| async move {
					node.volumes
						.list_library_volumes(library)
						.await
						.map_err(Into::into)
				}),
		)
		// .procedure(
		// 	"listByDevice",
		// 	R.with2(library())
		// 		.query(|(node, library), _: ()| async move {
		// 			node.volumes
		// 				.list_by_device(library)
		// 				.await
		// 				.map_err(Into::into)
		// 		}),
		// )
		.procedure(
			"unmount",
			R.with2(library())
				.mutation(|(node, _), fingerprint: Vec<u8>| async move {
					node.volumes
						.unmount_volume(VolumeFingerprint(fingerprint).into())
						.await
						.map_err(Into::into)
				}),
		)
		.procedure("events", {
			R.with2(library()).subscription(|(node, library), _: ()| {
				Ok(async_stream::stream! {
						let mut event_bus_rx = node.volumes.subscribe();

						while let Ok(event) = event_bus_rx.recv().await {
							yield event;
						}
				})
			})
		})
}
