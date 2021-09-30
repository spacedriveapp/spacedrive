use int_enum::IntEnum;
use serde::{Deserialize, Serialize};

#[repr(u8)]
#[derive(Clone, Copy, Debug, Serialize, Deserialize, IntEnum)]
pub enum Encryption {
  NONE = 0,
  AES128 = 1,
  AES192 = 2,
  AES256 = 3,
}

// impl From<i8> for Encryption {
//   fn from(val: i8) -> Self {
//     match val {
//       0 => Encryption::NONE,
//       1 => Encryption::AES128,
//       2 => Encryption::AES192,
//       3 => Encryption::AES256,
//     }
//   }
// }
