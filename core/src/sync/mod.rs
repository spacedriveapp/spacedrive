use core_derive::PropertyOperationApply;
use serde::{Deserialize, Serialize};

use self::{
  crdt::{CrdtCtx, PoMethod, ReplicateMethod},
  examples::{file::File, tag::Tag},
};

pub mod crdt;
pub mod engine;
pub mod examples;

// Property Operation
#[derive(PropertyOperationApply, Serialize, Deserialize, Debug, Clone)]
#[serde(rename = "po")]
pub enum PropertyOperation {
  Tag(PoMethod<Tag>),
  File(PoMethod<File>),
  // Job(PoMethod<Job>),
}

// Resource Replicate
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Replicate {
  FilePath(ReplicateMethod<File>),
  // Job(ReplicateMethod<Job>),
}

#[derive(Serialize, Deserialize, Debug)]
pub enum SyncMethod {
  // performs a property level operation on a resource
  // - records the change data in the database
  PropertyOperation(CrdtCtx<PropertyOperation>),
  // replicates the latest version of a resource by querying the database
  // - records timestamp in the database
  Replicate(CrdtCtx<Replicate>),
}

pub struct FakeCoreContext {}
