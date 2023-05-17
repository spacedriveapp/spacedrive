use crate::prisma::{object, PrismaClient};
use prisma_client_rust::not;
use sd_file_ext::kind::ObjectKind;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{sync::Arc, vec};

use strum_macros::{EnumString, EnumVariantNames};

/// Meow
#[derive(Serialize, Deserialize, Type, Debug, EnumVariantNames, EnumString)]
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
	Games,
	Books,
	// Contacts,
	// Movies,
	Trash,
}

impl Category {
	fn to_object_kind(&self) -> ObjectKind {
		match self {
			Category::Photos => ObjectKind::Image,
			Category::Videos => ObjectKind::Video,
			Category::Music => ObjectKind::Audio,
			Category::Books => ObjectKind::Book,
			Category::Encrypted => ObjectKind::Encrypted,
			_ => unimplemented!("Category::to_object_kind() for {:?}", self),
		}
	}
}

pub async fn get_category_count(db: &Arc<PrismaClient>, category: Category) -> i32 {
	let params = match category {
		Category::Recents => vec![not![object::date_accessed::equals(None)]],
		Category::Favorites => vec![object::favorite::equals(true)],
		Category::Photos
		| Category::Videos
		| Category::Music
		| Category::Encrypted
		| Category::Books => vec![object::kind::equals(category.to_object_kind() as i32)],
		Category::Downloads => {
			// TODO: Fetch the actual count for the Downloads category.
			return 0;
		}
		Category::Projects => {
			// TODO: Fetch the actual count for the Projects category.
			return 0;
		}
		Category::Games => {
			// TODO: Fetch the actual count for the Games category.
			return 0;
		}
		Category::Trash => {
			// TODO: Fetch the actual count for the Trash category.
			return 0;
		}
	};

	db.object().count(params).exec().await.unwrap_or(0) as i32
}
