use rspc::alpha::AlphaRouter;

use crate::sync::SyncMessage;

use super::{utils::library, Ctx, R};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("newMessage", {
			R.with2(library())
				.subscription(|(_, library), _: ()| async move {
					async_stream::stream! {
						let mut rx = library.sync.tx.subscribe();
						while let Ok(msg) = rx.recv().await {
							let op = match msg {
								SyncMessage::Ingested(op) => op,
								SyncMessage::Created(op) => op
							};
							yield op;
						}
					}
				})
		})
		.procedure("messages", {
			R.with2(library())
				.query(|(_, library), _: ()| async move { Ok(library.sync.get_ops().await?) })
		})
}
