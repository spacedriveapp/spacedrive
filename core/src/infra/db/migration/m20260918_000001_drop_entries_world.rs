//! Drop the entry-era substrate.
//!
//! Records, content, media facets, sidecars, and tags live in per-source
//! stores; navigation lives in Space items; pins are gone. Every table here
//! was verified empty of durable user data or already replaced before this
//! migration existed: the tag and metadata tables held zero rows on both
//! live libraries, entry rows stopped being written when the volume index
//! took over, and locations were removed without compatibility by decision.
//! There is no transfer step, so the drop is direct and runs in one
//! transaction: a failure leaves the library exactly as it was.
//!
//! Location-typed Space items and Locations groups are deleted rather than
//! migrated, per the same decision; Space items using the current navigation
//! contract are untouched.

use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, TransactionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Children before parents. With foreign keys enforced, dropping a parent
/// makes SQLite parse every child's constraints, and a child whose other
/// parent is already gone fails the whole statement.
const DROPPED_TABLES: &[&str] = &[
	"sync_generation",
	"entry_closure",
	"directory_paths",
	"image_media_data",
	"video_media_data",
	"audio_media_data",
	"sidecar_availability",
	"sidecar",
	"collection_entry",
	"collection",
	"location",
	"tag_closure",
	"tag_relationship",
	"tag_usage_pattern",
	"user_metadata_tag",
	"tag",
	"sync_conduit",
	"search_analytics",
	"indexer_rules",
	// FTS5 main table; SQLite drops its shadow tables with it. The explicit
	// shadow names below cover databases where a partial drop ever happened.
	"search_index",
	"search_index_config",
	"search_index_data",
	"search_index_docsize",
	"search_index_idx",
	"entries",
	"content_identities",
	"content_kinds",
	"mime_types",
	"user_metadata",
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		let txn = manager.get_connection().begin().await?;

		txn.execute_unprepared(
			"DELETE FROM space_items WHERE item_type LIKE '%\"Location\"%';
			 DELETE FROM space_groups WHERE group_type LIKE '%Locations%';",
		)
		.await?;

		for table in DROPPED_TABLES {
			txn.execute_unprepared(&format!("DROP TABLE IF EXISTS \"{table}\";"))
				.await?;
		}

		txn.commit().await
	}

	async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
		Err(DbErr::Custom(
			"the entry-era schema does not come back".to_string(),
		))
	}
}
