use int_enum::IntEnum;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[repr(i64)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, Eq, PartialEq, IntEnum)]
#[ts(export)]
pub enum EncryptionAlgorithm {
	None = 0,
	AES128 = 1,
	AES192 = 2,
	AES256 = 3,
}
