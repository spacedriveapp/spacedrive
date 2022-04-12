pub mod property_operation;
pub mod replicate;

use serde::{Deserialize, Serialize};

pub use self::{
  property_operation::{PoMethod, PropertyOperation},
  replicate::{Replicate, ReplicateMethod},
};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename = "cr")]
pub struct CrdtCtx<T> {
  #[serde(rename = "u")]
  pub uuid: String,
  #[serde(rename = "r")]
  pub resource: T,
}
