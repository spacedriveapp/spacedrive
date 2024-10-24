use crate::ModelId;

use prisma_client_rust::ModelTypes;
use serde::{de::DeserializeOwned, Serialize};

pub trait SyncId: Serialize + DeserializeOwned {
	type Model;
}

pub trait SyncModel: ModelTypes {
	const MODEL_ID: ModelId;
}

pub trait SharedSyncModel: SyncModel {
	type SyncId: SyncId;
}

pub trait RelationSyncId: SyncId {
	type ItemSyncId: SyncId;
	type GroupSyncId: SyncId;

	fn split(&self) -> (&Self::ItemSyncId, &Self::GroupSyncId);
}

pub trait RelationSyncModel: SyncModel {
	type SyncId: RelationSyncId;
}
