//! Resolve tag targets to assertion rows, grouped by the store that owns
//! them.
//!
//! Apply and unapply differ only in the `asserted` flag, so both resolve
//! here. A file target lands in the one store owning its path. A content
//! target lands once per store holding a copy: the content key reaches every
//! copy the store has, so the first copy anchors the claim there and the
//! rest inherit it at read time.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use sd_store::{Stamp, TagAssertion};
use uuid::Uuid;

use super::apply::input::TagTargets;
use crate::context::CoreContext;
use crate::ops::indexing::{store::SourceStore, VolumeIndex};

/// One store's slice of an application: the handle and the rows headed there.
pub struct StoreBatch {
	pub store: Arc<SourceStore>,
	pub rows: Vec<TagAssertion>,
}

/// Rows for a source another device owns: authored here, delivered through
/// the outbox when the owner answers.
pub struct RemoteBatch {
	pub device_uuid: Uuid,
	pub source_uuid: Uuid,
	pub rows: Vec<TagAssertion>,
}

pub struct ResolvedTargets {
	pub batches: Vec<StoreBatch>,
	pub remote: Vec<RemoteBatch>,
	/// Every record the claim reaches, for change events.
	pub affected: Vec<Uuid>,
	/// Targets that resolved to a store this device writes directly.
	pub resolved: u32,
	/// Targets that resolved to a remote-owned source and will deliver.
	pub pending: u32,
	pub warnings: Vec<String>,
}

pub async fn resolve(
	context: &Arc<CoreContext>,
	targets: &TagTargets,
	tags: &[Uuid],
	asserted: bool,
	stamp: &Stamp,
) -> ResolvedTargets {
	let cache = context.volume_index();
	let mut batches: HashMap<Uuid, StoreBatch> = HashMap::new();
	let mut remote: HashMap<Uuid, RemoteBatch> = HashMap::new();
	let mut affected = Vec::new();
	let mut resolved = 0u32;
	let mut pending = 0u32;
	let mut warnings = Vec::new();

	match targets {
		TagTargets::File(records) => {
			for &record in records {
				if let Some((store, external_id, content_uuid)) = locate(&cache, record).await {
					let batch = batches.entry(store.id()).or_insert_with(|| StoreBatch {
						store: store.clone(),
						rows: Vec::new(),
					});
					for &tag in tags {
						batch.rows.push(TagAssertion {
							tag_uuid: tag,
							record_uuid: record,
							external_id: external_id.clone(),
							content_uuid,
							asserted,
							stamp: stamp.clone(),
						});
					}
					affected.push(record);
					resolved += 1;
					continue;
				}

				if let Some((device, source, external_id, content_uuid)) =
					locate_replica(context, record).await
				{
					let batch = remote.entry(source).or_insert_with(|| RemoteBatch {
						device_uuid: device,
						source_uuid: source,
						rows: Vec::new(),
					});
					for &tag in tags {
						batch.rows.push(TagAssertion {
							tag_uuid: tag,
							record_uuid: record,
							external_id: external_id.clone(),
							content_uuid,
							asserted,
							stamp: stamp.clone(),
						});
					}
					affected.push(record);
					pending += 1;
					continue;
				}

				warnings.push(format!("no tracked source holds file {record}"));
			}
		}
		TagTargets::Content(contents) => {
			for &content in contents {
				let copies = cache.copies_of_content(content).await;
				if copies.is_empty() {
					warnings.push(format!("no indexed copy of content {content}"));
					continue;
				}

				let mut anchored_stores: HashSet<Uuid> = HashSet::new();
				for copy in &copies {
					let Some(store) = cache.store_for(&copy.path).await else {
						continue;
					};
					if !anchored_stores.insert(store.id()) {
						continue;
					}
					let external_id = store.external_id(&copy.path);
					let batch = batches.entry(store.id()).or_insert_with(|| StoreBatch {
						store: store.clone(),
						rows: Vec::new(),
					});
					for &tag in tags {
						batch.rows.push(TagAssertion {
							tag_uuid: tag,
							record_uuid: copy.record_uuid,
							external_id: external_id.clone(),
							content_uuid: Some(content),
							asserted,
							stamp: stamp.clone(),
						});
					}
				}

				if !anchored_stores.is_empty() {
					resolved += 1;
					affected.extend(copies.iter().map(|copy| copy.record_uuid));
					continue;
				}

				// No local copy of the bytes: anchor the claim in a replica's
				// owner instead, once per remote source holding a copy.
				let mut anchored_remote = false;
				for share in crate::service::mounts::peer::remote_shares().await {
					let Some(db) = crate::service::mounts::peer::open_replica_db(
						context,
						share.device_id,
						share.info.id,
					)
					.await
					else {
						continue;
					};
					let copies = sd_store::copies_of_content(db.pool(), content).await;
					db.pool().close().await;
					let copies = match copies {
						Ok(copies) => copies,
						Err(error) => {
							tracing::warn!(source = %share.info.id, %error, "replica content lookup failed");
							continue;
						}
					};
					let Some(anchor) = copies.first() else {
						continue;
					};
					let batch = remote.entry(share.info.id).or_insert_with(|| RemoteBatch {
						device_uuid: share.device_id,
						source_uuid: share.info.id,
						rows: Vec::new(),
					});
					for &tag in tags {
						batch.rows.push(TagAssertion {
							tag_uuid: tag,
							record_uuid: anchor.record_uuid,
							external_id: Some(anchor.external_id.clone()),
							content_uuid: Some(content),
							asserted,
							stamp: stamp.clone(),
						});
					}
					affected.extend(copies.iter().map(|copy| copy.record_uuid));
					anchored_remote = true;
				}

				if anchored_remote {
					pending += 1;
				} else {
					warnings.push(format!("no tracked source holds content {content}"));
				}
			}
		}
	}

	ResolvedTargets {
		batches: batches.into_values().collect(),
		remote: remote.into_values().collect(),
		affected,
		resolved,
		pending,
		warnings,
	}
}

/// The replica holding a record, when no local store does: the owner and
/// source to deliver to, with the evidence the assertion carries.
///
/// A snapshot-delivered replica answers from its loaded arena; a nested
/// source delivered as a database answers from the file. The content key may
/// be unknown here; the owner's merge binds it from its own store.
async fn locate_replica(
	context: &Arc<CoreContext>,
	record: Uuid,
) -> Option<(Uuid, Uuid, Option<String>, Option<Uuid>)> {
	for share in crate::service::mounts::peer::remote_shares().await {
		let arena_path = { share.index.read().await.get_path_by_uuid(record) };
		if let Some(path) = arena_path {
			let relative = path.strip_prefix(&share.info.root).ok().map(|relative| {
				relative
					.to_string_lossy()
					.replace(std::path::MAIN_SEPARATOR, "/")
			});
			let mut content_uuid = None;
			if let Some(db) = crate::service::mounts::peer::open_replica_db(
				context,
				share.device_id,
				share.info.id,
			)
			.await
			{
				if let Ok(Some(entry)) = db.entry_by_uuid(record).await {
					content_uuid = entry.content_uuid;
				}
				db.pool().close().await;
			}
			return Some((share.device_id, share.info.id, relative, content_uuid));
		}

		let Some(db) =
			crate::service::mounts::peer::open_replica_db(context, share.device_id, share.info.id)
				.await
		else {
			continue;
		};
		let looked_up = db.entry_by_uuid(record).await;
		db.pool().close().await;
		match looked_up {
			Ok(Some(entry)) => {
				return Some((
					share.device_id,
					share.info.id,
					Some(entry.relative_path),
					entry.content_uuid,
				));
			}
			Ok(None) => {}
			Err(error) => {
				tracing::warn!(source = %share.info.id, %error, "replica record lookup failed")
			}
		}
	}
	None
}

/// The store owning a record, with the evidence an assertion carries. The
/// arena answers fast when loaded; the stores are the floor when it is not,
/// so tagging works on a cold daemon and on a detached source.
async fn locate(
	cache: &VolumeIndex,
	record: Uuid,
) -> Option<(Arc<SourceStore>, Option<String>, Option<Uuid>)> {
	if let Some(path) = cache.path_of_record(record).await {
		if let Some(store) = cache.store_for(&path).await {
			let external_id = store.external_id(&path);
			let content_uuid = store.content_of(record).await;
			return Some((store, external_id, content_uuid));
		}
	}

	for store in cache.stores().await {
		match store.db().entry_by_uuid(record).await {
			Ok(Some(entry)) => {
				return Some((store, Some(entry.relative_path), entry.content_uuid));
			}
			Ok(None) => {}
			Err(error) => {
				tracing::warn!(source = %store.id(), %error, "record lookup failed")
			}
		}
	}
	None
}
