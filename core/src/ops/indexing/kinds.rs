//! Naming stored content rows after the kinds loaded extensions declare.
//!
//! The content identity phase writes a row's kind when it reads the bytes.
//! Rows identified before an extension existed carry a built-in kind, or
//! nothing at all, and the bytes are not read again for them: one update
//! per declared kind names every row whose file carries one of the kind's
//! extensions. The pass runs when a store opens, so every store sees the
//! kinds loaded at startup, and when an extension with kinds loads while
//! stores are open. It is idempotent, so running it twice costs one indexed
//! probe per kind and changes nothing.

use std::collections::HashMap;
use std::sync::Arc;

use uuid::Uuid;

use crate::domain::{ContentKind, File};
use crate::filetype::FileTypeRegistry;
use crate::ops::indexing::{SourceStore, VolumeIndex};

/// Name the rows of one store after every extension kind the registry
/// holds. Returns how many rows gained a name.
pub async fn name_kinds_in_store(
	db: &sd_store::SourceDb,
	registry: &FileTypeRegistry,
) -> Result<u64, sd_store::Error> {
	let mut named = 0;
	for (kind, extensions) in registry.extension_kinds() {
		let Some(kind_name) = kind.kind_name.as_deref() else {
			continue;
		};
		named += sd_store::name_content_kind_by_extension(
			db.pool(),
			kind_name,
			kind.category as i32 as i64,
			&extensions,
		)
		.await?;
	}
	Ok(named)
}

/// Run the pass over every store this machine has open.
pub async fn name_kinds_in_open_stores(volume_index: &Arc<VolumeIndex>) {
	let registry = FileTypeRegistry::current();
	if registry.extension_kinds().is_empty() {
		return;
	}
	for store in volume_index.open_stores() {
		match name_kinds_in_store(store.db(), &registry).await {
			Ok(0) => {}
			Ok(named) => {
				tracing::info!(source = %store.id(), named, "named content rows after extension kinds")
			}
			Err(error) => {
				tracing::warn!(source = %store.id(), %error, "could not name content rows after extension kinds")
			}
		}
	}
}

/// Give files listed from an arena the kind their store rows carry. The
/// arena holds one built-in kind per entry, derived from the name while it
/// was walked; the store holds what the content identity phase found,
/// including an extension kind's name, which is what the client resolves a
/// preview and a label from and what survives the extension's unload.
pub async fn decorate_kinds(volume_index: &VolumeIndex, files: &mut [File]) {
	let mut by_store: HashMap<Uuid, (Arc<SourceStore>, Vec<usize>)> = HashMap::new();
	for (position, file) in files.iter().enumerate() {
		if file.kind != crate::domain::EntryKind::File {
			continue;
		}
		let Some(path) = file.sd_path.as_local_path() else {
			continue;
		};
		let Some(store) = volume_index.store_for(path).await else {
			continue;
		};
		by_store
			.entry(store.id())
			.or_insert_with(|| (store.clone(), Vec::new()))
			.1
			.push(position);
	}

	for (_, (store, positions)) in by_store {
		let ids: Vec<Uuid> = positions.iter().map(|&p| files[p].id).collect();
		let rows = match sd_store::read::content_kinds_for_records(store.db().pool(), &ids).await {
			Ok(rows) => rows,
			Err(error) => {
				tracing::warn!(source = %store.id(), %error, "content kinds unavailable for listing");
				continue;
			}
		};
		let by_record: HashMap<Uuid, (Option<i64>, Option<String>)> = rows
			.into_iter()
			.map(|(uuid, kind, name)| (uuid, (kind, name)))
			.collect();
		for &position in &positions {
			let Some((kind, name)) = by_record.get(&files[position].id) else {
				continue;
			};
			if let Some(kind) = kind.and_then(|k| ContentKind::try_from(k as i32).ok()) {
				files[position].content_kind = kind;
			}
			files[position].content_kind_name = name.clone();
		}
	}
}
