use crate::library::Library;

use sd_prisma::{prisma::tag, prisma_sync};
use sd_sync::*;

use chrono::{DateTime, FixedOffset, Utc};

use sd_utils::msgpack;
use serde::Deserialize;
use specta::Type;
use uuid::Uuid;

pub mod seed;

#[derive(Type, Deserialize, Clone)]
pub struct TagCreateArgs {
	pub name: String,
	pub color: String,
}

impl TagCreateArgs {
	pub async fn exec(
		self,
		Library { db, sync, .. }: &Library,
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
						(tag::name::NAME, msgpack!(&self.name)),
						(tag::color::NAME, msgpack!(&self.color)),
						(tag::is_hidden::NAME, msgpack!(false)),
						(
							tag::date_created::NAME,
							msgpack!(&date_created.to_rfc3339()),
						),
					],
				),
				db.tag().create(
					pub_id,
					vec![
						tag::name::set(Some(self.name)),
						tag::color::set(Some(self.color)),
						tag::is_hidden::set(Some(false)),
						tag::date_created::set(Some(date_created)),
					],
				),
			),
		)
		.await
	}
}
