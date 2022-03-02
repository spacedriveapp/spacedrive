use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Shard {
    pub shard_type: ShardType,
    pub client_id: u32,
    pub library_id: u32,
    pub timestamp: DateTime<Utc>,
    pub sql: Option<String>,
}

enum ShardType {
    Create,
    Mutate,
    Delete,
}

impl Shard {
    pub fn new(shard_type: ShardType, sql: Option<String>) -> Self {
        Self { shard_type, sql }
    }
}

fn main() {
    // example
    Shard::new(
        ShardType::Mutate,
        file::Model::update_many()
            .set(pear)
            .filter(file::Column::Id.eq(1)),
    );
}
