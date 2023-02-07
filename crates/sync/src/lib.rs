mod crdt;

pub use crdt::*;

use prisma_client_rust::ModelTypes;
use serde::{de::DeserializeOwned, Serialize};

pub trait SyncId: Serialize + DeserializeOwned {
	type ModelTypes: SyncType;
}

pub trait SyncType: ModelTypes {
	type SyncId: SyncId;
	type Marker: SyncTypeMarker;
}

pub trait SyncTypeMarker {}

pub struct LocalSyncType;
impl SyncTypeMarker for LocalSyncType {}

pub struct OwnedSyncType;
impl SyncTypeMarker for OwnedSyncType {}

pub struct SharedSyncType;
impl SyncTypeMarker for SharedSyncType {}

pub struct RelationSyncType;
impl SyncTypeMarker for RelationSyncType {}
