pub mod seed;

use chrono::Utc;
use serde::Deserialize;
use serde_json::json;
use specta::Type;

use uuid::Uuid;

use crate::{library::Library, prisma::tag, sync};

#[derive(Type, Deserialize)]
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

		sync.write_op(
			db,
			sync.unique_shared_create(
				sync::tag::SyncId {
					pub_id: pub_id.clone(),
				},
				[
					(tag::name::NAME, json!(&self.name)),
					(tag::color::NAME, json!(&self.color)),
				],
			),
			db.tag().create(
				pub_id,
				vec![
					tag::name::set(Some(self.name)),
					tag::color::set(Some(self.color)),
					tag::date_created::set(Some(Utc::now().into())),
				],
			),
		)
		.await
	}
}
