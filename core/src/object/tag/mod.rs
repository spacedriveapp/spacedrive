pub mod seed;

use chrono::{DateTime, FixedOffset, Utc};
use sd_prisma::prisma_sync;
use sd_sync::*;
use serde::Deserialize;
use serde_json::json;
use specta::Type;

use uuid::Uuid;

use crate::{library::LoadedLibrary, prisma::tag};

#[derive(Type, Deserialize, Clone)]
pub struct TagCreateArgs {
	pub name: String,
	pub color: String,
}

impl TagCreateArgs {
	pub async fn exec(
		self,
		LoadedLibrary { db, sync, .. }: &LoadedLibrary,
	) -> prisma_client_rust::Result<tag::Data> {
		let pub_id = Uuid::new_v4().as_bytes().to_vec();
		let date_created: DateTime<FixedOffset> = Utc::now().into();

		sync.write_ops(
			db,
			(
				sync.shared_create(
					prisma_sync::tag::SyncId {
						pub_id: pub_id.clone(),
					},
					[
						(tag::name::NAME, json!(&self.name)),
						(tag::color::NAME, json!(&self.color)),
						(tag::date_created::NAME, json!(&date_created.to_rfc3339())),
					],
				),
				db.tag().create(
					pub_id,
					vec![
						tag::name::set(Some(self.name)),
						tag::color::set(Some(self.color)),
						tag::date_created::set(Some(date_created)),
					],
				),
			),
		)
		.await
	}
}
