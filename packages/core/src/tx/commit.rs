use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Commit {
    pub commit_type: CommitType,
    pub client_id: u32,
    pub library_id: u32,
    pub timestamp: DateTime<Utc>,
    pub sql: Option<String>,
}

enum CommitType {
    Create,
    Mutate,
    Delete,
}

impl Commit {
    pub fn new(commit_type: CommitType, sql: Option<String>) -> Self {
        Self { commit_type, sql }
    }

    pub fn from_query<T: SerializeQuery>(query: T) -> Self {
        Self::new(CommitType::Mutate, query.serialize_query())
    }
}

struct RawQuery(String);

trait SerializeQuery {
    fn serialize_query(self) -> String;
}

struct PostFindMany {
    query: String,
}

impl SerializeQuery for PostFindUnique {
    fn serialize_query(self) -> String {
        RawQuery(self.query)
    }
}

fn main() {
    // example
    Commit::from_query(
        client
            .post()
            .find_unique(Post::id().equals("post0".to_string()))
            .with(vec![Post::user().fetch()]),
    );
}
