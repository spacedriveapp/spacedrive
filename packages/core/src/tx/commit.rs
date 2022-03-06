use chrono::{DateTime, Utc};
use prisma_client_rust::SerializeQuery;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::state;

// an SQL commit to be sent to connected clients
#[derive(Serialize, Deserialize)]
pub struct Commit {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub client_uuid: String,
    pub library_uuid: String,
    pub query: String,
}

impl Commit {
    pub fn new(query: String) -> Self {
        let client = state::client::get();
        let id = Uuid::new_v4().to_string();
        let timestamp = Utc::now();
        Self {
            id,
            sql,
            client_uuid: client.client_id,
            library_uuid: client.current_library_id,
            timestamp,
        }
    }

    pub fn from_query<T: SerializeQuery>(query: T) -> Self {
        Self::new(query.serialize_query())
    }
}

// example
fn do_something() {
    Commit::new(db.client().create_one(
        Client::uuid().set(config.client_id.clone()),
        Client::name().set(hostname.clone()),
        vec![],
    ))
}
