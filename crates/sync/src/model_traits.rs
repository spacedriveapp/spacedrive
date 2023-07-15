use prisma_client_rust::ModelTypes;
use serde::{de::DeserializeOwned, Serialize};

pub trait SyncId: Serialize + DeserializeOwned {
	type Model: ModelTypes;
}

pub trait LocalSyncModel: ModelTypes {
	type SyncId: SyncId;
}

pub trait SharedSyncModel: ModelTypes {
	type SyncId: SyncId;
}

pub trait RelationSyncId: SyncId {
	type ItemSyncId: SyncId;
	type GroupSyncId: SyncId;

	fn split(&self) -> (&Self::ItemSyncId, &Self::GroupSyncId);
}

pub trait RelationSyncModel: ModelTypes {
	type SyncId: RelationSyncId;
}
