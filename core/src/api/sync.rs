use std::sync::atomic::Ordering;

use sd_core_sync::GetOpsArgs;

use rspc::alpha::AlphaRouter;

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
		.procedure("enable", {
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
					.await;

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
}
