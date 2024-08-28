use crate::Node;

use sd_core_sync::SyncManager;

use std::sync::Arc;

use uuid::Uuid;

pub mod sync;

#[derive(Default)]
pub struct State {
	pub sync: sync::State,
}

pub async fn start(
	node: &Arc<Node>,
	actors: &Arc<sd_actors::Actors>,
	library_id: Uuid,
	instance_uuid: Uuid,
	sync: &Arc<SyncManager>,
	db: &Arc<sd_prisma::prisma::PrismaClient>,
) -> State {
	let sync = sync::declare_actors(
		node,
		actors,
		library_id,
		instance_uuid,
		sync.clone(),
		db.clone(),
	)
	.await;

	State { sync }
}
