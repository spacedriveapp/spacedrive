use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub enum Action {
  SCAN_DIR,
  ENCRYPT_FILE,
  UPLOAD_FILE,
}

// A job is triggered by a user interaction or schedule
#[derive(Serialize, Deserialize)]
pub struct Job {
  pub id: String,
  pub client_id: String,
  pub storage_device_id: Option<String>,
  pub uri: Option<String>,
  pub action: Action,
  pub status: String,
  pub complete: bool,
}

// // A task is a way to track the completion of a portion of a job
// // usually specific
// #[derive(Serialize, Deserialize)]
// pub struct Task {
//   pub id: String,
//   pub job_id: String,
// }
