use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use tracing::{debug, error};

use crate::thumbnail::ThumbKey;

use sd_core_prisma_helpers::{file_path_for_frontend, label_with_objects, object_with_file_paths};

#[derive(Serialize, Type, Debug)]
#[serde(tag = "type")]
pub enum ExplorerItem {
	Path {
		// provide the frontend with the thumbnail key explicitly
		thumbnail: Option<ThumbKey>,
		// this tells the frontend if a thumbnail actually exists or not
		has_created_thumbnail: bool,
		// we can't actually modify data from PCR types, thats why computed properties are used on ExplorerItem
		item: Box<file_path_for_frontend::Data>,
	},
	Object {
		thumbnail: Option<ThumbKey>,
		has_created_thumbnail: bool,
		item: object_with_file_paths::Data,
	},
	NonIndexedPath {
		thumbnail: Option<ThumbKey>,
		has_created_thumbnail: bool,
		item: NonIndexedPathItem,
	},
	Location {
		item: location::Data,
	},
	SpacedropPeer {
		item: PeerMetadata,
	},
	Label {
		thumbnails: Vec<ThumbKey>,
		item: label_with_objects::Data,
	},
}

impl ExplorerItem {
	pub fn id(&self) -> String {
		let ty = match self {
			ExplorerItem::Path { .. } => "FilePath",
			ExplorerItem::Object { .. } => "Object",
			ExplorerItem::Location { .. } => "Location",
			ExplorerItem::NonIndexedPath { .. } => "NonIndexedPath",
			ExplorerItem::SpacedropPeer { .. } => "SpacedropPeer",
			ExplorerItem::Label { .. } => "Label",
		};
		match self {
			ExplorerItem::Path { item, .. } => format!("{ty}:{}", item.id),
			ExplorerItem::Object { item, .. } => format!("{ty}:{}", item.id),
			ExplorerItem::Location { item, .. } => format!("{ty}:{}", item.id),
			ExplorerItem::NonIndexedPath { item, .. } => format!("{ty}:{}", item.path),
			ExplorerItem::SpacedropPeer { item, .. } => format!("{ty}:{}", item.name), // TODO: Use a proper primary key
			ExplorerItem::Label { item, .. } => format!("{ty}:{}", item.name),
		}
	}
}

#[derive(Serialize, Type, Debug)]
pub struct NonIndexedPathItem {
	pub path: String,
	pub name: String,
	pub extension: String,
	pub kind: i32,
	pub is_dir: bool,
	pub date_created: DateTime<Utc>,
	pub date_modified: DateTime<Utc>,
	pub size_in_bytes_bytes: Vec<u8>,
	pub hidden: bool,
}
