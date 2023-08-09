use rspc::alpha::AlphaRouter;
use sd_core_sync::GetOpsArgs;

use super::{utils::library, Ctx, R};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("newMessage", {
			R.with2(library())
				.subscription(|(_, library), _: ()| async move {
					async_stream::stream! {
						let mut rx = library.sync.tx.subscribe();
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
}
