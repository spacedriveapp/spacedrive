pub enum SpacedrivePath {
	Location(u64, PathBuf),
	Virtual(PathBuf),
	NonIndexed(PathBuf),
}

pub struct ExplorerItem {
	pub id: u64,
	pub pub_id: Bytes,
	pub inode: Option<u64>,
	// the unique Object for this item
	pub object_id: Option<u64>,
	// the path of this item in Spacedrive
	pub path: SpacedrivePath,

	// metadata about this item
	pub name: String,
	pub extension: Option<String>,
	pub kind: ObjectKind,
	pub size: Option<u64>,
	pub date_created: Option<DateTime<Utc>>,
	pub date_modified: Option<DateTime<Utc>>,
	pub date_indexed: Option<DateTime<Utc>>,
	pub is_dir: bool,
	pub is_hidden: bool,
	pub key_id: Option<u64>,

	// computed properties
	pub thumbnail: Option<ThumbKey>,
	pub has_created_thumbnail: bool,
	pub duplicate_paths: Vec<SpacedrivePath>,
}
