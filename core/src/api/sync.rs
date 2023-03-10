use super::{utils::LibraryRequest, RouterBuilder};
use crate::sync::SyncMessage;

pub fn mount() -> RouterBuilder {
	RouterBuilder::new().library_subscription("messages", |t| {
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
}
