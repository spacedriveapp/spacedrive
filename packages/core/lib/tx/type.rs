

pub enum TransactionType {
    CREATE,
    UPDATE,
    DELETE,
    
}

pub struct Transaction {
    pub id: i32,
    pub timestamp: i32,
    pub client_id: i32
    pub target_client_id: i32,
    type TransactionType
}