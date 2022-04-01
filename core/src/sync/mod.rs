//

pub trait Synchronizer<D> {
	async fn create(&self, resource: D) -> Result<()>;
	async fn update<FV>(&self, field: String, data: FV) -> Result<()>;
	async fn delete(&self, id: String) -> Result<()>;
}

pub struct SyncEvent {
    pub uuid: String,
    pub resource_identifier: String,
    pub client_id: String,
    pub timestamp: DateTime<Utc>,
    pub action: SyncAction,
}

pub enum SyncAction {
    Create,
    Update,
    Delete,
}