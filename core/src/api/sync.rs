use rspc::alpha::AlphaRouter;
use sd_core_sync::GetOpsArgs;
use std::sync::atomic::Ordering;

use crate::util::MaybeUndefined;

use super::{utils::library, Ctx, R};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("newMessage", {
			R.with2(library())
				.subscription(|(_, library), _: ()| async move {
					async_stream::stream! {
						let mut rx = library.sync.subscribe();
						while let Ok(_msg) = rx.recv().await {
							// let op = match msg {
							// 	SyncMessage::Ingested => (),
							// 	SyncMessage::Created => op
							// };
							yield ();
						}
					}
				})
		})
		.procedure("messages", {
			R.with2(library()).query(|(_, library), _: ()| async move {
				Ok(library
					.sync
					.get_ops(GetOpsArgs {
						clocks: vec![],
						count: 1000,
					})
					.await?)
			})
		})
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

					sd_core_sync::backfill::backfill_operations(
						&library.db,
						&library.sync,
						library.config().await.instance_id,
					)
					.await?;

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
						let cloud_sync = &library.cloud.sync;
						let sync = &library.sync.shared;

						loop {
							yield Data {
							  ingest: sync.active.load(Ordering::Relaxed),
								cloud_send: cloud_sync.send_active.load(Ordering::Relaxed),
								cloud_receive: cloud_sync.receive_active.load(Ordering::Relaxed),
								cloud_ingest: cloud_sync.ingest_active.load(Ordering::Relaxed),
							};

							tokio::select! {
								_ = cloud_sync.notifier.notified() => {},
								_ = sync.active_notify.notified() => {}
							}
						}
					}
				})
		})
}
