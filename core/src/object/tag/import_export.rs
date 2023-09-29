use crate::util::import_export_manager::{ImportExport, ImportExportError};
use crate::{library::Library, prisma::tag};
use async_trait::async_trait;
use futures::stream::{self, StreamExt};
use sd_prisma::prisma::{file_path, tag_on_object};
use sd_prisma::prisma_sync;
use serde::{Deserialize, Serialize};
use serde_json::json;
use specta::Type;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct JsonTag {
	pub_id: String,
	name: Option<String>,
	color: Option<String>,
	objects: Vec<JsonTagObject>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct JsonTagObject {
	pub_id: String,
	cas_ids: Vec<String>,
}

#[derive(Deserialize, Clone, Type)]
pub struct TagExportArgs {
	pub tag_id: i32,
}

#[async_trait]
impl ImportExport for JsonTag {
	type ExportContext = TagExportArgs;
	type ImportData = JsonTag;

	async fn export(
		context: Self::ExportContext,
		Library { db, .. }: &Library,
	) -> Result<Self::ImportData, ImportExportError> {
		let tag = db
			.tag()
			.find_unique(tag::id::equals(context.tag_id))
			.exec()
			.await?
			.ok_or(ImportExportError::NotFound)?;

		let tags_on_object = db
			.tag_on_object()
			.find_many(vec![tag_on_object::tag_id::equals(tag.id)])
			.select(tag_on_object::select!({
				object: select {
					id
					pub_id
				}
				tag: select {
					id
					pub_id
					name
					color
				}
			}))
			.exec()
			.await?;

		let combined_results: Vec<_> = stream::iter(tags_on_object.into_iter())
			.filter_map(|to| {
				let db = db.clone();
				async move {
					let file_paths = db
						.file_path()
						.find_many(vec![file_path::object_id::equals(Some(to.object.id))])
						.select(file_path::select!({ cas_id }))
						.exec()
						.await
						.ok()?;

					let mut cas_ids: Vec<_> =
						file_paths.into_iter().filter_map(|fp| fp.cas_id).collect();

					cas_ids.sort();
					cas_ids.dedup();
					Some((to, cas_ids))
				}
			})
			.collect()
			.await;

		// Create JsonTag objects from the fetched and processed data
		let json_tags = combined_results
			.into_iter()
			.map(|(to, cas_ids)| JsonTagObject {
				pub_id: hex::encode(to.object.pub_id),
				cas_ids,
			})
			.collect::<Vec<JsonTagObject>>();

		Ok(JsonTag {
			pub_id: hex::encode(tag.pub_id),
			name: tag.name,
			color: tag.color,
			objects: json_tags,
		})
	}

	async fn import(data: Self::ImportData, library: &Library) -> Result<(), ImportExportError> {
		let Library { db, sync, .. } = library;

		// let existing_tag = db
		// 	.tag()
		// 	.find_unique(tag::pub_id::equals(
		// 		hex::decode(&data.pub_id).unwrap_or_default(),
		// 	))
		// 	.exec()
		// 	.await?;

		// let tag_id = if let Some(existing_tag) = existing_tag {
		// 	existing_tag.id
		// } else {
		// 	let new_tag = sync.shared_create(
		// 		prisma_sync::tag::SyncId {
		// 			pub_id: hex::decode(&data.pub_id).unwrap_or_default(),
		// 		},
		// 		vec![
		// 			(tag::name::NAME, json!(data.name)),
		// 			(tag::color::NAME, json!(data.color)),
		// 			// Add other fields as necessary
		// 		],
		// 	);

		// 	sync.write_ops(
		// 		db,
		// 		(
		// 			vec![new_tag.clone()],
		// 			db.tag().create(tag::Create {
		// 				pub_id: hex::decode(&data.pub_id).unwrap_or_default(),
		// 				name: data.name.clone(),
		// 				color: data.color.clone(),
		// 				// Add other fields as necessary
		// 			}),
		// 		),
		// 	)
		// 	.await?
		// 	.id // Assuming a field to get the ID of the created tag, adjust as necessary
		// };

		// for obj in &data.objects {
		// 	// Here, we match cas_id to ensure the correct linkage
		// 	let file_path = db
		// 		.file_path()
		// 		.find_many(file_path::cas_id::in_vec(&obj.cas_ids))
		// 		.exec()
		// 		.await?;

		// 	for path in file_path {
		// 		if let Some(object_id) = path.object_id {
		// 			let existing_relation = db
		// 				.tag_on_object()
		// 				.find_unique(
		// 					tag_on_object::tag_id::equals(tag_id)
		// 						.and(tag_on_object::object_id::equals(object_id)),
		// 				)
		// 				.exec()
		// 				.await?;

		// 			if existing_relation.is_none() {
		// 				let new_relation = sync.relation_create(
		// 					prisma_sync::tag_on_object::SyncId {
		// 						tag: prisma_sync::tag::SyncId {
		// 							pub_id: hex::decode(&data.pub_id).unwrap_or_default(),
		// 						},
		// 						object: prisma_sync::object::SyncId {
		// 							pub_id: hex::decode(&obj.pub_id).unwrap_or_default(),
		// 						},
		// 					},
		// 					vec![],
		// 				);

		// 				sync.write_ops(
		// 					db,
		// 					(
		// 						vec![new_relation],
		// 						db.tag_on_object().create(tag_on_object::Create {
		// 							tag_id,
		// 							object_id,
		// 							// Populate other fields as necessary
		// 						}),
		// 					),
		// 				)
		// 				.await?;
		// 			}
		// 		}
		// 	}
		// }

		Ok(())
	}
}
