use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum Encryption {
  NONE,
  AES128,
  AES192,
  AES256,
}
