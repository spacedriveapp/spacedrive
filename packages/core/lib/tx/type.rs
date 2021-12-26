//
// Transactions are used to describe changes to be made to the database
// - a transaction must be run as a single read, write, or delete operation to the SQLite database
// - they are emitted by a given client and accepted or rejected by sister clients
// - if a client rejects a transaction the entire database will be marked for re-sync
pub struct Transaction {
    pub id: i32,
    pub timestamp: i32, // unix timestamp
    pub client_id: i32, // the client that created the transaction

    pub model: String, // the model that the transaction is for
    pub method: TransactionMethod,

    // vector of transaction entries
    pub mutations: Option<Vec<ObjectMutation>>,
}

//
pub struct ObjectMutation {
    pub primary_key: Vec<i32>,
    pub columns: Vec<String>,
    pub new_values: Vec<String>,
}

pub enum TransactionMethod {
    CREATE,
    UPDATE,
    DELETE,
}

// create tag
// update tag
// assign tag to file
// create files
// create action records
//
