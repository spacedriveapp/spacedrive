//! Fill `File.tags` from the stores that own each file.
//!
//! Assertions live only in SQLite: the arena never carries them, so even a
//! listing served from memory decorates through the store. One batched read
//! per source keeps that a join rather than a query per row.

use std::collections::HashMap;
use std::sync::Arc;

use uuid::Uuid;

use crate::context::CoreContext;
use crate::domain::{File, Tag};
use crate::library::Library;
use crate::ops::indexing::{store::SourceStore, VolumeIndex};
use crate::ops::tags::outbox;
use crate::service::mounts::peer::{self, RemoteShare};

/// Decorate files already known to live in one store, one batched read.
pub async fn decorate_from_store(db: &sd_store::SourceDb, files: &mut [File]) {
	let ids: Vec<Uuid> = files.iter().map(|file| file.id).collect();
	let state = match db.tags_for_records(&ids).await {
		Ok(state) => state,
		Err(error) => {
			tracing::warn!(%error, "tag decoration unavailable");
			return;
		}
	};
	if state.is_empty() {
		return;
	}
	for file in files {
		if let Some(applied) = state.get(&file.id) {
			file.tags = applied.iter().map(Tag::from_applied).collect();
		}
	}
}

/// Decorate files of one replica: the owner's tags as of the generation its
/// database was delivered at, with this device's claims still awaiting the
/// owner's ack on top.
pub async fn decorate_replica(
	context: &Arc<CoreContext>,
	library_id: Uuid,
	share: &RemoteShare,
	files: &mut [File],
) {
	if let Some(db) = peer::open_replica_db(context, share.device_id, share.info.id).await {
		decorate_from_store(&db, files).await;
		db.pool().close().await;
	}
	if let Some(library) = context.libraries().await.get_library(library_id).await {
		overlay_pending(&library, share.info.id, files).await;
	}
}

/// Ride pending outbox claims on top of what a replica shows, so a tag
/// applied while the owner is away appears immediately and honestly: the
/// assertion exists and is authored, only delivery is pending. Batches apply
/// in authored order; the overlay disappears as acks retire the rows and the
/// next delivered generation carries the state itself.
pub async fn overlay_pending(library: &Library, source_uuid: Uuid, files: &mut [File]) {
	let pending = outbox::pending_for_source(library, source_uuid).await;
	if pending.is_empty() {
		return;
	}

	let mut by_record: HashMap<Uuid, usize> = HashMap::new();
	for (position, file) in files.iter().enumerate() {
		by_record.insert(file.id, position);
	}

	for batch in pending {
		let definitions: HashMap<Uuid, Tag> = batch
			.definitions
			.iter()
			.map(|wire| {
				(
					wire.uuid,
					Tag {
						id: wire.uuid,
						name: crate::domain::tag::leaf_of(&wire.path),
						path: wire.path.clone(),
						color: wire.color.clone(),
						icon: wire.icon.clone(),
					},
				)
			})
			.collect();

		for assertion in &batch.assertions {
			let Some(&position) = by_record.get(&assertion.record_uuid) else {
				continue;
			};
			let tags = &mut files[position].tags;
			tags.retain(|tag| tag.id != assertion.tag_uuid);
			if assertion.asserted {
				if let Some(tag) = definitions.get(&assertion.tag_uuid) {
					tags.push(tag.clone());
				}
			}
		}
	}
}

pub async fn decorate_files(cache: &VolumeIndex, files: &mut [File]) {
	let mut by_store: HashMap<Uuid, (Arc<SourceStore>, Vec<usize>)> = HashMap::new();
	for (position, file) in files.iter().enumerate() {
		let Some(path) = file.sd_path.as_local_path() else {
			continue;
		};
		let Some(store) = cache.store_for(path).await else {
			continue;
		};
		by_store
			.entry(store.id())
			.or_insert_with(|| (store.clone(), Vec::new()))
			.1
			.push(position);
	}

	for (_, (store, positions)) in by_store {
		let ids: Vec<Uuid> = positions
			.iter()
			.map(|&position| files[position].id)
			.collect();
		let state = match store.db().tags_for_records(&ids).await {
			Ok(state) => state,
			Err(error) => {
				tracing::warn!(source = %store.id(), %error, "tag decoration unavailable");
				continue;
			}
		};
		if state.is_empty() {
			continue;
		}
		for &position in &positions {
			if let Some(applied) = state.get(&files[position].id) {
				files[position].tags = applied.iter().map(Tag::from_applied).collect();
			}
		}
	}
}
