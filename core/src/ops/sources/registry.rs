//! The `sources` table read as what it is: the registry.
//!
//! One row per source whatever fills it, so a walk and an adapter are listed,
//! counted and deleted the same way. What forks them is `data_type`, matching
//! `_schema.data_type_id` in the source's own store.
//!
//! The archive engine holds stores and adapters and is told which source it is
//! working on. It used to keep its own list in `registry.db`, which meant two
//! lists of sources that could not see each other and drifted whenever one was
//! written without the other.

use crate::infra::db::entities::source;
use anyhow::{Context, Result};
use chrono::Utc;
use sd_archive::SourceRef;
use sd_store::TrustTier;
use sea_orm::{
	ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
};
use uuid::Uuid;

/// The `data_type` of a source produced by walking a filesystem.
pub const FILESYSTEM: &str = "filesystem";

/// Every registration in this library, newest first.
pub async fn all(db: &DatabaseConnection) -> Result<Vec<source::Model>> {
	source::Entity::find()
		.order_by_desc(source::Column::CreatedAt)
		.all(db)
		.await
		.context("list sources")
}

/// The sources an adapter fills, which are the ones the engine can act on.
pub async fn adapter_backed(db: &DatabaseConnection) -> Result<Vec<source::Model>> {
	Ok(all(db)
		.await?
		.into_iter()
		.filter(|row| row.adapter_id.is_some())
		.collect())
}

/// One registration by uuid.
pub async fn get(db: &DatabaseConnection, id: Uuid) -> Result<source::Model> {
	source::Entity::find()
		.filter(source::Column::Uuid.eq(id))
		.one(db)
		.await
		.context("read source")?
		.with_context(|| format!("no source {id}"))
}

/// A registration as the engine needs to see it.
///
/// Only meaningful for an adapter-backed source: a filesystem source has a walk
/// rather than an adapter, and nothing in the engine can act on one.
pub fn source_ref(row: &source::Model) -> Option<SourceRef> {
	Some(SourceRef {
		id: store_id(row.uuid),
		name: row.name.clone(),
		data_type: row.data_type.clone(),
		adapter_id: row.adapter_id.clone()?,
		config: serde_json::from_str(&row.config).unwrap_or(serde_json::Value::Null),
		trust_tier: TrustTier::from_str_or_default(&row.trust_tier),
	})
}

/// Every adapter-backed registration, ready for the engine.
pub async fn source_refs(db: &DatabaseConnection) -> Result<Vec<SourceRef>> {
	Ok(adapter_backed(db)
		.await?
		.iter()
		.filter_map(source_ref)
		.collect())
}

/// How a source's store is named on disk. Shared with filesystem sources, whose
/// stores sit in the same directory.
pub fn store_id(uuid: Uuid) -> String {
	uuid.simple().to_string()
}

/// The store name for a source id as a client sends it.
pub fn parse_store_id(id: &str) -> Result<String> {
	Ok(store_id(
		Uuid::parse_str(id).with_context(|| format!("not a source id: {id}"))?,
	))
}

/// A read-only handle on a filesystem source's store, wherever placement put
/// it, or `None` for a source the volume index does not register here: an
/// adapter source, whose store the engine opens from the in-library layout.
///
/// A registered filesystem source never falls through to the engine. Its
/// store may be on the drive, and asking the in-library layout for it would
/// answer with a different, empty database once anything created that
/// directory.
pub async fn filesystem_store(
	context: &crate::context::CoreContext,
	id: &str,
) -> Result<Option<std::sync::Arc<sd_store::SourceDb>>> {
	let uuid = Uuid::parse_str(id).with_context(|| format!("not a source id: {id}"))?;
	let cache = context.volume_index();
	if cache.source_root(uuid).is_none() {
		return Ok(None);
	}
	cache
		.read_store(uuid)
		.await
		.map(Some)
		.with_context(|| format!("the store of source {id} is not available on this machine"))
}

/// Write a registration for a source an adapter fills.
pub async fn register(
	db: &DatabaseConnection,
	uuid: Uuid,
	name: &str,
	adapter_id: &str,
	config: &serde_json::Value,
	facts: &sd_archive::AdapterFacts,
) -> Result<source::Model> {
	let now = Utc::now();
	let row = source::ActiveModel {
		uuid: Set(uuid),
		name: Set(name.to_string()),
		data_type: Set(facts.data_type.clone()),
		adapter_id: Set(Some(adapter_id.to_string())),
		config: Set(config.to_string()),
		root: Set(None),
		volume_uuid: Set(None),
		record_count: Set(Some(0)),
		directory_count: Set(None),
		total_bytes: Set(None),
		unique_bytes: Set(None),
		last_indexed_at: Set(None),
		status: Set("idle".to_string()),
		trust_tier: Set(facts.trust_tier.as_str().to_string()),
		created_at: Set(now),
		last_seen_at: Set(now),
		..Default::default()
	};

	source::Entity::insert(row)
		.exec(db)
		.await
		.context("register source")?;

	get(db, uuid).await
}

/// Record what a sync run did.
///
/// The engine answers with the run and this writes it down, because the
/// registration is here rather than there.
pub async fn record_run(
	db: &DatabaseConnection,
	uuid: Uuid,
	status: &str,
	record_count: Option<i64>,
) -> Result<()> {
	let now = Utc::now();
	source::Entity::update_many()
		.filter(source::Column::Uuid.eq(uuid))
		.set(source::ActiveModel {
			status: Set(status.to_string()),
			record_count: Set(record_count),
			last_indexed_at: Set(Some(now)),
			last_seen_at: Set(now),
			..Default::default()
		})
		.exec(db)
		.await
		.context("record sync run")?;
	Ok(())
}

/// Mark a source as busy, so a listing says so while a sync runs.
pub async fn mark_status(db: &DatabaseConnection, uuid: Uuid, status: &str) -> Result<()> {
	source::Entity::update_many()
		.filter(source::Column::Uuid.eq(uuid))
		.set(source::ActiveModel {
			status: Set(status.to_string()),
			..Default::default()
		})
		.exec(db)
		.await
		.context("update source status")?;
	Ok(())
}

/// Remove a registration. The store it named is the engine's to delete.
pub async fn unregister(db: &DatabaseConnection, uuid: Uuid) -> Result<()> {
	source::Entity::delete_many()
		.filter(source::Column::Uuid.eq(uuid))
		.exec(db)
		.await
		.context("unregister source")?;
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::infra::db::Database;
	use sd_store::TrustTier;
	use std::sync::Arc;

	async fn library(dir: &std::path::Path) -> Arc<Database> {
		let db = Database::create(&dir.join("library.db"))
			.await
			.expect("create library");
		db.migrate().await.expect("migrate");
		Arc::new(db)
	}

	fn facts() -> sd_archive::AdapterFacts {
		sd_archive::AdapterFacts {
			data_type: "mail".to_string(),
			trust_tier: TrustTier::External,
		}
	}

	/// A walk and an adapter are the same kind of row, listed together and told
	/// apart by `data_type`. Two lists is what this replaced.
	#[tokio::test]
	async fn both_kinds_of_source_are_one_list() {
		let dir = tempfile::tempdir().unwrap();
		let db = library(dir.path()).await;

		let adapter = Uuid::now_v7();
		register(
			db.conn(),
			adapter,
			"Inbox",
			"gmail",
			&serde_json::json!({"account": "me"}),
			&facts(),
		)
		.await
		.expect("register");

		let walked = Uuid::now_v7();
		source::Entity::insert(source::ActiveModel {
			uuid: Set(walked),
			name: Set("Home".to_string()),
			data_type: Set(FILESYSTEM.to_string()),
			adapter_id: Set(None),
			config: Set("{}".to_string()),
			root: Set(Some("Users/me".to_string())),
			status: Set("idle".to_string()),
			trust_tier: Set(TrustTier::Authored.as_str().to_string()),
			created_at: Set(Utc::now()),
			last_seen_at: Set(Utc::now()),
			..Default::default()
		})
		.exec(db.conn())
		.await
		.expect("insert filesystem source");

		let all = all(db.conn()).await.expect("list");
		assert_eq!(all.len(), 2);

		// Only one of them has an adapter to act on, and that is the whole
		// difference between them as far as the engine is concerned.
		let refs = source_refs(db.conn()).await.expect("refs");
		assert_eq!(refs.len(), 1);
		assert_eq!(refs[0].id, store_id(adapter));
		assert_eq!(refs[0].adapter_id, "gmail");
		assert_eq!(refs[0].config["account"], "me");
	}

	#[tokio::test]
	async fn a_sync_run_is_recorded_against_the_registration() {
		let dir = tempfile::tempdir().unwrap();
		let db = library(dir.path()).await;
		let id = Uuid::now_v7();
		register(
			db.conn(),
			id,
			"Inbox",
			"gmail",
			&serde_json::Value::Null,
			&facts(),
		)
		.await
		.expect("register");

		mark_status(db.conn(), id, "syncing").await.expect("status");
		assert_eq!(get(db.conn(), id).await.expect("get").status, "syncing");

		record_run(db.conn(), id, "idle", Some(42))
			.await
			.expect("record");

		let row = get(db.conn(), id).await.expect("get");
		assert_eq!(row.status, "idle");
		assert_eq!(row.record_count, Some(42));
		assert!(row.last_indexed_at.is_some());

		unregister(db.conn(), id).await.expect("unregister");
		assert!(get(db.conn(), id).await.is_err());
	}
}
