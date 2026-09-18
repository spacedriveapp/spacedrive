//! Files carrying a tag.
//!
//! A tag reaches a file two ways: applied to that copy, or applied to the
//! bytes and therefore to every copy of them. Both live in the assertion
//! tables of each source store, so this fans out across the stores, resolves
//! state per record, and builds files from the arena.

use crate::{
	context::CoreContext,
	domain::{addressing::SdPath, File},
	infra::query::{LibraryQuery, QueryError, QueryResult},
	ops::tags::{decorate, definitions},
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashSet;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetFilesByTagInput {
	pub tag_id: Uuid,
	/// Tagging something `Camera` should find what was tagged
	/// `Camera/Leica`, so children are included by default in clients.
	pub include_children: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetFilesByTagOutput {
	pub files: Vec<File>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetFilesByTagQuery {
	pub input: GetFilesByTagInput,
}

impl LibraryQuery for GetFilesByTagQuery {
	type Input = GetFilesByTagInput;
	type Output = GetFilesByTagOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let library_id = session
			.current_library_id
			.ok_or_else(|| QueryError::Internal("No library in session".to_string()))?;
		let library = context
			.libraries()
			.await
			.get_library(library_id)
			.await
			.ok_or_else(|| QueryError::Internal("Library not found".to_string()))?;
		let cache = context.ephemeral_cache();

		let all = definitions::all(&library, &cache).await;
		let Some(target) = all.iter().find(|d| d.uuid == self.input.tag_id) else {
			return Ok(GetFilesByTagOutput { files: Vec::new() });
		};

		let mut wanted = vec![target.uuid];
		if self.input.include_children {
			let prefix = format!("{}/", target.path);
			wanted.extend(
				all.iter()
					.filter(|d| d.path.starts_with(&prefix))
					.map(|d| d.uuid),
			);
		}

		// Serve entirely from the stores that answered: an assertion names a
		// record, and resolving it must not depend on a loaded arena, or a
		// cold daemon and a detached source would both answer empty. A record
		// can carry the tag directly and through its bytes; it is one file
		// either way.
		let device_slug = crate::device::get_current_device_slug();
		let mut seen: HashSet<Uuid> = HashSet::new();
		let mut files = Vec::new();
		for store in cache.stores().await {
			let mut records = Vec::new();
			for &tag in &wanted {
				match store.db().records_with_tag(tag).await {
					Ok(tagged) => records.extend(tagged),
					Err(error) => {
						tracing::warn!(source = %store.id(), %error, "tagged records unavailable");
					}
				}
			}

			let mut batch = Vec::new();
			for record in records {
				if !seen.insert(record) {
					continue;
				}
				match store.db().entry_by_uuid(record).await {
					Ok(Some(entry)) => {
						let sd_path = SdPath::Physical {
							device_slug: device_slug.clone(),
							path: store.root().join(&entry.relative_path),
						};
						batch.push(File::from_store_entry(&entry, sd_path));
					}
					// An orphaned assertion waits for rebind rather than
					// surfacing a record nothing can resolve.
					Ok(None) => {}
					Err(error) => {
						tracing::warn!(source = %store.id(), %error, "record lookup failed")
					}
				}
			}
			decorate::decorate_from_store(store.db(), &mut batch).await;
			files.append(&mut batch);
		}

		files.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

		Ok(GetFilesByTagOutput { files })
	}
}

crate::register_library_query!(GetFilesByTagQuery, "files.by_tag");
