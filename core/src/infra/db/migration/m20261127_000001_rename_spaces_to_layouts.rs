//! Renames the sidebar's space tables to layouts.
//!
//! "Space" now names a folder with policy that agents and people work in, so
//! the sidebar's saved arrangements become layouts. Rows, ids and uuids are
//! kept; only table and column names change. The indexes are recreated under
//! layout names because SQLite cannot rename an index.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.get_connection()
			.execute_unprepared(
				"ALTER TABLE spaces RENAME TO layouts;
				 ALTER TABLE space_groups RENAME TO layout_groups;
				 ALTER TABLE space_items RENAME TO layout_items;
				 ALTER TABLE layout_groups RENAME COLUMN space_id TO layout_id;
				 ALTER TABLE layout_items RENAME COLUMN space_id TO layout_id;
				 DROP INDEX IF EXISTS idx_spaces_order;
				 DROP INDEX IF EXISTS idx_space_groups_space_id;
				 DROP INDEX IF EXISTS idx_space_groups_order;
				 DROP INDEX IF EXISTS idx_space_items_space_id;
				 DROP INDEX IF EXISTS idx_space_items_group_id;
				 DROP INDEX IF EXISTS idx_space_items_order;
				 DROP INDEX IF EXISTS idx_space_items_entry_uuid;
				 CREATE INDEX IF NOT EXISTS idx_layout_items_entry_uuid ON layout_items(entry_uuid);
				 CREATE INDEX IF NOT EXISTS idx_layouts_order ON layouts(\"order\");
				 CREATE INDEX IF NOT EXISTS idx_layout_groups_layout_id ON layout_groups(layout_id);
				 CREATE INDEX IF NOT EXISTS idx_layout_groups_order ON layout_groups(\"order\");
				 CREATE INDEX IF NOT EXISTS idx_layout_items_layout_id ON layout_items(layout_id);
				 CREATE INDEX IF NOT EXISTS idx_layout_items_group_id ON layout_items(group_id);
				 CREATE INDEX IF NOT EXISTS idx_layout_items_order ON layout_items(\"order\");",
			)
			.await?;
		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.get_connection()
			.execute_unprepared(
				"DROP INDEX IF EXISTS idx_layout_items_entry_uuid;
				 DROP INDEX IF EXISTS idx_layouts_order;
				 DROP INDEX IF EXISTS idx_layout_groups_layout_id;
				 DROP INDEX IF EXISTS idx_layout_groups_order;
				 DROP INDEX IF EXISTS idx_layout_items_layout_id;
				 DROP INDEX IF EXISTS idx_layout_items_group_id;
				 DROP INDEX IF EXISTS idx_layout_items_order;
				 ALTER TABLE layout_items RENAME COLUMN layout_id TO space_id;
				 ALTER TABLE layout_groups RENAME COLUMN layout_id TO space_id;
				 ALTER TABLE layout_items RENAME TO space_items;
				 ALTER TABLE layout_groups RENAME TO space_groups;
				 ALTER TABLE layouts RENAME TO spaces;",
			)
			.await?;
		Ok(())
	}
}
