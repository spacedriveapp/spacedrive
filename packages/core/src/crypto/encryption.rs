use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Serialize, Deserialize, Clone, Debug, TS)]
pub enum Encryption {
	None = 0,
	AES128,
	AES192,
	AES256,
}
