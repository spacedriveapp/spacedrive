use std::{sync::Arc, time::Duration};
use tokio::sync::mpsc::*;
use tracing::{debug, error};

use crate::prisma::*;

pub fn start(db: Arc<PrismaClient>) -> Sender<()> {
	let (tx, mut rx) = channel(4);

	tokio::spawn(async move {
		while let Some(()) = rx.recv().await {
			tokio::time::sleep(Duration::from_millis(10)).await;

			let Ok(objs) = db
				.object()
				.find_many(vec![object::file_paths::none(vec![])])
				.take(512)
				.select(object::select!({ id pub_id }))
				.exec()
				.await else {
                    continue;
                };

			if objs.is_empty() {
				continue;
			}

			debug!("Removing {} orphaned objects", objs.len());

			let ids: Vec<_> = objs.iter().map(|o| o.id).collect();

			if let Err(e) = db
				._batch((
					db.tag_on_object()
						.delete_many(vec![tag_on_object::object_id::in_vec(ids.clone())]),
					db.object()
						.delete_many(vec![object::id::in_vec(ids.clone())]),
				))
				.await
			{
				error!("Failed to remove orphaned objects: {e}");
			}
		}
	});

	tx
}
