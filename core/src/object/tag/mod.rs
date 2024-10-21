use crate::library::Library;

use sd_prisma::{prisma::tag, prisma_sync};
use sd_sync::*;

use chrono::Utc;
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
	) -> Result<tag::Data, sd_core_sync::Error> {
		let pub_id = Uuid::now_v7().as_bytes().to_vec();

		let (sync_params, db_params) = [
			sync_db_entry!(self.name, tag::name),
			sync_db_entry!(self.color, tag::color),
			sync_db_entry!(false, tag::is_hidden),
			sync_db_entry!(Utc::now(), tag::date_created),
		]
		.into_iter()
		.unzip::<_, _, Vec<_>, Vec<_>>();

		sync.write_op(
			db,
			sync.shared_create(
				prisma_sync::tag::SyncId {
					pub_id: pub_id.clone(),
				},
				sync_params,
			),
			db.tag().create(pub_id, db_params),
		)
		.await
	}
}
