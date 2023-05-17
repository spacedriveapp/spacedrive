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

pub async fn get_category_count(db: &Arc<PrismaClient>, category: Category) -> i32 {
	match category {
		Category::Recents => db
			.object()
			.count(vec![not![object::date_accessed::equals(None)]])
			.exec()
			.await
			.unwrap_or(0) as i32,
		Category::Favorites => db
			.object()
			.count(vec![object::favorite::equals(true)])
			.exec()
			.await
			.unwrap_or(0) as i32,
		Category::Photos
		| Category::Videos
		| Category::Music
		| Category::Encrypted
		| Category::Books => db
			.object()
			.count(vec![object::kind::equals(match category {
				Category::Photos => ObjectKind::Image as i32,
				Category::Videos => ObjectKind::Video as i32,
				Category::Music => ObjectKind::Audio as i32,
				Category::Books => ObjectKind::Book as i32,
				Category::Encrypted => ObjectKind::Encrypted as i32,
				_ => unreachable!(),
			})])
			.exec()
			.await
			.unwrap_or(0) as i32,
		Category::Downloads => {
			// TODO: Fetch the actual count for the Downloads category.
			0
		}
		Category::Projects => {
			// TODO: Fetch the actual count for the Projects category.
			0
		}
		Category::Games => {
			// TODO: Fetch the actual count for the Games category.
			0
		}
		Category::Trash => {
			// TODO: Fetch the actual count for the Trash category.
			0
		}
	}
}
