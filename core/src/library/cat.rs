use crate::prisma::{object, PrismaClient};
use prisma_client_rust::not;
use sd_file_ext::kind::ObjectKind;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{sync::Arc, vec};

use strum_macros::{EnumString, EnumVariantNames};

/// Meow
#[derive(
	Serialize,
	Deserialize,
	Type,
	Debug,
	PartialEq,
	Eq,
	PartialOrd,
	Ord,
	EnumVariantNames,
	EnumString,
	Clone,
	Copy,
)]
pub enum Category {
	Recents,
	Favorites,
	Photos,
	Videos,
	Movies,
	Music,
	Documents,
	Downloads,
	Encrypted,
	Projects,
	Applications,
	Archives,
	Databases,
	Games,
	Books,
	Contacts,
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
	let param = match category {
		Category::Recents => not![object::date_accessed::equals(None)],
		Category::Favorites => object::favorite::equals(true),
		Category::Photos
		| Category::Videos
		| Category::Music
		| Category::Encrypted
		| Category::Books => object::kind::equals(category.to_object_kind() as i32),
		_ => return 0,
	};

	db.object().count(vec![param]).exec().await.unwrap_or(0) as i32
}
