use std::{sync::Arc, time::Duration};
use tokio::sync::mpsc::*;
use tracing::{debug, error};

use crate::prisma::*;

// Actor that can be invoked to find and delete objects with no matching file paths
#[derive(Clone)]
pub struct OrphanRemoverActor {
	tx: Sender<()>,
}

impl OrphanRemoverActor {
	pub fn spawn(db: Arc<PrismaClient>) -> Self {
		let (tx, mut rx) = channel(4);

		tokio::spawn({
			let tx = tx.clone();
			async move {
				tx.send(()).await.ok();

				while let Some(()) = rx.recv().await {
					// prevents timeouts
					tokio::time::sleep(Duration::from_millis(10)).await;

					loop {
						let objs = match db
							.object()
							.find_many(vec![object::file_paths::none(vec![])])
							.take(512)
							.select(object::select!({ id pub_id }))
							.exec()
							.await
						{
							Ok(objs) => objs,
							Err(e) => {
								error!("Failed to fetch orphaned objects: {e}");
								break;
							}
						};

						if objs.is_empty() {
							break;
						}

						debug!("Removing {} orphaned objects", objs.len());

						let ids: Vec<_> = objs.iter().map(|o| o.id).collect();

						if let Err(e) = db
							._batch((
								db.tag_on_object().delete_many(vec![
									tag_on_object::object_id::in_vec(ids.clone()),
								]),
								db.object().delete_many(vec![object::id::in_vec(ids)]),
							))
							.await
						{
							error!("Failed to remove orphaned objects: {e}");
						}
					}
				}
			}
		});

		Self { tx }
	}

	pub async fn invoke(&self) {
		self.tx.send(()).await.ok();
	}
}
