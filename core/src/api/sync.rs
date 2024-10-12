use rspc::alpha::AlphaRouter;
use std::sync::atomic::Ordering;

use crate::util::MaybeUndefined;

use super::{utils::library, Ctx, R};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("backfill", {
			R.with2(library())
				.mutation(|(node, library), _: ()| async move {
					if library
						.config()
						.await
						.generate_sync_operations
						.load(Ordering::Relaxed)
					{
						return Ok(());
					}

					sd_core_sync::backfill::backfill_operations(&library.sync).await?;

					node.libraries
						.edit(
							library.id,
							None,
							MaybeUndefined::Undefined,
							MaybeUndefined::Undefined,
							Some(true),
						)
						.await?;

					Ok(())
				})
		})
		.procedure("enabled", {
			R.with2(library()).query(|(_, library), _: ()| async move {
				Ok(library
					.config()
					.await
					.generate_sync_operations
					.load(Ordering::Relaxed))
			})
		})
		.procedure("active", {
			R.with2(library())
				.subscription(|(_, library), _: ()| async move {
					#[derive(serde::Serialize, specta::Type)]
					#[specta(rename = "SyncStatus")]
					struct Data {
						ingest: bool,
						cloud_send: bool,
						cloud_receive: bool,
						cloud_ingest: bool,
					}

					async_stream::stream! {
						let cloud_sync_state = &library.cloud_sync_state;
						let sync = &library.sync;

						loop {
							yield Data {
							  ingest: sync.active.load(Ordering::Relaxed),
								cloud_send: cloud_sync_state.send_active.load(Ordering::Relaxed),
								cloud_receive: cloud_sync_state.receive_active.load(Ordering::Relaxed),
								cloud_ingest: cloud_sync_state.ingest_active.load(Ordering::Relaxed),
							};

							tokio::select! {
								_ = cloud_sync_state.state_change_notifier.notified() => {},
								_ = sync.active_notify.notified() => {}
							}
						}
					}
				})
		})
}
