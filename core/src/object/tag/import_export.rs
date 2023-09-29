use crate::util::import_export_manager::{ImportExport, ImportExportError};
use crate::{library::Library, prisma::tag};
use async_trait::async_trait;
use futures::stream::{self, StreamExt};
use sd_prisma::prisma::{file_path, tag_on_object};
use serde::{Deserialize, Serialize};
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
pub struct TagImportExport {
	pub tag_id: i32,
}

#[async_trait]
impl ImportExport<JsonTag> for TagImportExport {
	async fn export(&self, Library { db, .. }: &Library) -> Result<JsonTag, ImportExportError> {
		let tag = db
			.tag()
			.find_unique(tag::id::equals(self.tag_id))
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
	async fn import(&self, lib: &Library) -> Result<JsonTag, ImportExportError> {
		unimplemented!()
	}
}
