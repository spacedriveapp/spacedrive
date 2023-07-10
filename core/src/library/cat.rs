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
	// this should really be done without unimplemented! and on another type but ehh
	fn to_object_kind(self) -> ObjectKind {
		match self {
			Category::Photos => ObjectKind::Image,
			Category::Videos => ObjectKind::Video,
			Category::Music => ObjectKind::Audio,
			Category::Books => ObjectKind::Book,
			Category::Encrypted => ObjectKind::Encrypted,
			_ => unimplemented!("Category::to_object_kind() for {:?}", self),
		}
	}

	pub fn to_where_param(self) -> object::WhereParam {
		match self {
			Category::Recents => not![object::date_accessed::equals(None)],
			Category::Favorites => object::favorite::equals(Some(true)),
			Category::Photos
			| Category::Videos
			| Category::Music
			| Category::Encrypted
			| Category::Books => object::kind::equals(Some(self.to_object_kind() as i32)),
			_ => object::id::equals(-1),
		}
	}
}

pub async fn get_category_count(db: &Arc<PrismaClient>, category: Category) -> i32 {
	db.object()
		.count(vec![category.to_where_param()])
		.exec()
		.await
		.unwrap_or(0) as i32
}
