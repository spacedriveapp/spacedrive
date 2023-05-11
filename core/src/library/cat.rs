use serde::{Deserialize, Serialize};
use specta::Type;
use strum_macros::EnumVariantNames;

/// Meow
#[derive(Serialize, Deserialize, Type, Debug, EnumVariantNames)]
#[serde(tag = "type")]
pub enum Category {
	Recents,
	Favorites,
	Photos,
	Videos,
	Music,
	// Documents,
	Downloads,
	Encrypted,
	Projects,
	// Applications,
	// Archives,
	// Databases
	// Games,
	// Books,
	// Contacts,
	// Movies,
	// Trash,
}
