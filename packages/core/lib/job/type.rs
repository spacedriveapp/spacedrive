


pub struct Job {
    pub id: i32,
    pub timestamp: i32,
    pub client_id: i32
    pub target_client_id: i32,
    pub file_id: i32,
    pub action: Action,
    pub status: String,
    pub complete: bool,
}