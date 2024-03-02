use sd_prisma::prisma::{file_path, object};

use serde::{Deserialize, Serialize};
use specta::Type;

pub mod cas;
pub mod old_file_identifier;
pub mod fs;
pub mod media;
pub mod old_orphan_remover;
pub mod tag;
pub mod validation;

// Objects are primarily created by the identifier from Paths
// Some Objects are purely virtual, unless they have one or more associated Paths, which refer to a file found in a Location
// Objects are what can be added to Spaces

// Object selectables!
object::select!(object_for_file_identifier {
	pub_id
	file_paths: select { pub_id cas_id extension is_dir materialized_path name }
});

// The response to provide the Explorer when looking at Objects
#[derive(Debug, Serialize, Deserialize, Type)]
pub struct ObjectsForExplorer {
	pub objects: Vec<ObjectData>,
	// pub context: ExplorerContext,
}

#[derive(Debug, Serialize, Deserialize, Type)]
pub enum ObjectData {
	Object(Box<object::Data>),
	Path(Box<file_path::Data>),
}
