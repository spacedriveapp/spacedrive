use crate::sync::SyncMessage;

use super::{utils::LibraryRequest, RouterBuilder};

pub fn mount() -> RouterBuilder {
	RouterBuilder::new()
		.library_subscription("newMessage", |t| {
			t(|ctx, _: (), library_id| {
				async_stream::stream! {
					let Some(lib) = ctx.library_manager.get_ctx(library_id).await else {
						return
					};
					let mut rx = lib.sync.tx.subscribe();
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
		.library_query("messages", |t| {
			t(|_, _: (), library| async move { Ok(library.sync.get_ops().await?) })
		})
}
