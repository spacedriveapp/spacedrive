use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		let db = manager.get_connection();

		// Remove duplicate (user_metadata_id, tag_id) pairs, keeping the newest (MAX id)
		// which has the most recent version/updated_at/device_uuid state.
		// This must run before creating the unique index.
		db.execute_unprepared(
			"DELETE FROM user_metadata_tag \
			 WHERE id NOT IN ( \
			     SELECT MAX(id) FROM user_metadata_tag \
			     GROUP BY user_metadata_id, tag_id \
			 )",
		)
		.await?;

		// Add unique index so the pair can never be duplicated again.
		manager
			.create_index(
				Index::create()
					.if_not_exists()
					.name("idx_umt_unique_pair")
					.table(Alias::new("user_metadata_tag"))
					.col(Alias::new("user_metadata_id"))
					.col(Alias::new("tag_id"))
					.unique()
					.to_owned(),
			)
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.drop_index(
				Index::drop()
					.name("idx_umt_unique_pair")
					.table(Alias::new("user_metadata_tag"))
					.to_owned(),
			)
			.await?;

		Ok(())
	}
}
