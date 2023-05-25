use thiserror::Error;
use uuid::Uuid;

use crate::{prisma::*, util::db::uuid_to_bytes};

// Be careful!

pub const FAVORITES_TAG: SystemTag = SystemTag {
	pub_id: 0,
	name: "Favorites",
	color: "FAC607",
};

const SYSTEM_TAGS: [SystemTag; 1] = [FAVORITES_TAG];

#[derive(Error, Debug)]
pub enum SeedError {
	#[error("An error occurred with the database while applying migrations: {0}")]
	DatabaseError(#[from] prisma_client_rust::QueryError),
	#[error("Failed to seed tags, ")]
	TagsExist,
}

impl SystemTag {
	fn to_params(self) -> Vec<tag::UncheckedSetParam> {
		vec![
			tag::name::set(Some(self.name.to_string())),
			tag::color::set(Some(self.color.to_string())),
		]
	}
}

pub struct SystemTag {
	pub pub_id: u8,
	pub name: &'static str,
	pub color: &'static str,
}

/// None of the code here should EVER be modified under ANY circumstance
///
/// You better know what the hell you're doing.
/// If system tag IDs get messed up then it's your head.
mod secret_internals_do_not_modify_or_you_will_be_fired {
	use super::*;

	const MAX_ID: u8 = u8::MAX;

	pub fn generate_non_system_id() -> Uuid {
		loop {
			let id = Uuid::new_v4();
			if !is_system_id(id) {
				return id;
			}
		}
	}

	pub fn is_system_id(id: Uuid) -> bool {
		return id.as_u128() <= MAX_ID as u128;
	}

	pub async fn seed(db: &PrismaClient) -> Result<(), SeedError> {
		db.tag()
			.create_many(
				SYSTEM_TAGS
					.into_iter()
					.map(|tag| {
						tag::create_unchecked(
							uuid_to_bytes(Uuid::from_u128(tag.pub_id as u128)),
							tag.to_params(),
						)
					})
					.collect(),
			)
			// safe bc this stuff happens outside the sync system
			.skip_duplicates()
			.exec()
			.await?;

		Ok(())
	}
}

pub use secret_internals_do_not_modify_or_you_will_be_fired::*;
