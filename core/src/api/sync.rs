use sd_core_sync::GetOpsArgs;

use super::{utils::library, Ctx, RouterBuilder, R};

pub(crate) fn mount() -> RouterBuilder {
	R.router()
		.procedure("newMessage", {
			R.with(library())
				.subscription(|(_, library), _: ()| async move {
					async_stream::stream! {
						let mut rx = library.sync.tx.subscribe();
						while let Ok(_msg) = rx.recv().await {
							// let op = match msg {
							// 	SyncMessage::Ingested => (),
							// 	SyncMessage::Created => op
							// };
							yield Ok(());
						}
					}
				})
		})
		.procedure("messages", {
			R.with(library()).query(|(_, library), _: ()| async move {
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
