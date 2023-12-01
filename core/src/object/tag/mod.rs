use crate::{library::Library, prisma::tag};

use sd_prisma::prisma_sync;
use sd_sync::*;

#[cfg(feature = "skynet")]
use sd_prisma::prisma::{file_path, tag_on_object};

#[cfg(feature = "skynet")]
use std::collections::HashSet;

use chrono::{DateTime, FixedOffset, Utc};
use int_enum::IntEnum;
use serde::{Deserialize, Serialize};
use serde_json::json;
use specta::Type;
use uuid::Uuid;

pub mod seed;

#[repr(i32)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, Eq, PartialEq, IntEnum)]
#[non_exhaustive]
pub enum TagKind {
	UserAssigned = 0,
	Label = 1,
	Category = 2,
}

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
						(tag::name::NAME, json!(&self.name)),
						(tag::color::NAME, json!(&self.color)),
						(tag::kind::NAME, json!(TagKind::UserAssigned as i32)),
						(tag::is_hidden::NAME, json!(false)),
						(tag::date_created::NAME, json!(&date_created.to_rfc3339())),
					],
				),
				db.tag().create(
					pub_id,
					vec![
						tag::name::set(Some(self.name)),
						tag::color::set(Some(self.color)),
						tag::kind::set(Some(TagKind::UserAssigned as i32)),
						tag::is_hidden::set(Some(false)),
						tag::date_created::set(Some(date_created)),
					],
				),
			),
		)
		.await
	}
}

#[cfg(feature = "skynet")]
pub async fn assign_labels(
	object_id: file_path::id::Type,
	mut labels: HashSet<String>,
	Library { db, sync, .. }: &Library,
) -> Result<(), prisma_client_rust::QueryError> {
	let mut labels_ids = db
		.tag()
		.find_many(vec![
			tag::name::in_vec(labels.iter().cloned().collect()),
			tag::kind::equals(Some(TagKind::Label as i32)),
		])
		.select(tag::select!({ id name }))
		.exec()
		.await?
		.into_iter()
		.map(|tag| {
			if let Some(name) = tag.name {
				labels.remove(&name);
			}
			tag.id
		})
		.collect::<Vec<_>>();

	let date_created: DateTime<FixedOffset> = Utc::now().into();

	if !labels.is_empty() {
		let (sync_stuff, queries) = labels
			.into_iter()
			.map(|name| {
				let pub_id = Uuid::new_v4().as_bytes().to_vec();

				(
					sync.shared_create(
						prisma_sync::tag::SyncId {
							pub_id: pub_id.clone(),
						},
						[
							(tag::name::NAME, json!(&name)),
							(tag::color::NAME, json!(null)),
							(tag::kind::NAME, json!(TagKind::Label as i32)),
							(tag::is_hidden::NAME, json!(false)),
							(tag::date_created::NAME, json!(&date_created.to_rfc3339())),
						],
					),
					db.tag()
						.create(
							pub_id,
							vec![
								tag::name::set(Some(name)),
								tag::color::set(None),
								tag::kind::set(Some(TagKind::Label as i32)),
								tag::is_hidden::set(Some(false)),
								tag::date_created::set(Some(date_created)),
							],
						)
						.select(tag::select!({ id })),
				)
			})
			.unzip::<_, _, Vec<_>, Vec<_>>();

		labels_ids.extend(
			sync.write_ops(db, (sync_stuff.into_iter().flatten().collect(), queries))
				.await?
				.into_iter()
				.map(|tag| tag.id),
		);
	}

	db.tag_on_object()
		.create_many(
			labels_ids
				.into_iter()
				.map(|label_id| {
					tag_on_object::create_unchecked(
						label_id,
						object_id,
						vec![tag_on_object::date_created::set(Some(date_created))],
					)
				})
				.collect(),
		)
		.skip_duplicates()
		.exec()
		.await?;

	Ok(())
}
