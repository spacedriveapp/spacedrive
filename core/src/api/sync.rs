use sd_core_sync::GetOpsArgs;

use rspc::alpha::AlphaRouter;

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
				.mutation(|(_, library), _: ()| async move {
					sd_core_sync::backfill::backfill_operations(
						&library.db,
						&library.sync,
						library.config().await.instance_id,
					)
					.await;

					Ok(())
				})
		})
}
