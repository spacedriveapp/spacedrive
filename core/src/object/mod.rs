pub mod cas;
pub mod identifier_job;
pub mod preview;

// Objects are primarily created by the identifier from Paths
// Some Objects are purely virtual, unless they have one or more associated Paths, which refer to a file found in a Location
// Objects are what can be added to Spaces

use rspc::Type;
use serde::{Deserialize, Serialize};

use crate::prisma;

// The response to provide the Explorer when looking at Objects
#[derive(Debug, Serialize, Deserialize, Type)]
pub struct ObjectsForExplorer {
	pub objects: Vec<ObjectData>,
	// pub context: ExplorerContext,
}

// #[derive(Debug, Serialize, Deserialize, Type)]
// pub enum ExplorerContext {
// 	Location(Box<prisma::file_path::Data>),
// 	Space(Box<prisma::file::Data>),
// 	Tag(Box<prisma::file::Data>),
// 	// Search(Box<prisma::file_path::Data>),
// }

#[derive(Debug, Serialize, Deserialize, Type)]
pub enum ObjectData {
	Object(Box<prisma::object::Data>),
	Path(Box<prisma::file_path::Data>),
}
