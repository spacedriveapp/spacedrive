use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
	fn name(&self) -> &str {
		"m202501111_000001_create_spaces"
	}
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Create spaces table
		manager
			.create_table(
				Table::create()
					.table(Spaces::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(Spaces::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(ColumnDef::new(Spaces::Uuid).uuid().not_null().unique_key())
					.col(ColumnDef::new(Spaces::Name).string().not_null())
					.col(ColumnDef::new(Spaces::Icon).string().not_null())
					.col(ColumnDef::new(Spaces::Color).string().not_null())
					.col(
						ColumnDef::new(Spaces::Order)
							.integer()
							.not_null()
							.default(0),
					)
					.col(
						ColumnDef::new(Spaces::CreatedAt)
							.timestamp()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.col(
						ColumnDef::new(Spaces::UpdatedAt)
							.timestamp()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.to_owned(),
			)
			.await?;

		// Create space_groups table
		manager
			.create_table(
				Table::create()
					.table(SpaceGroups::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(SpaceGroups::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(
						ColumnDef::new(SpaceGroups::Uuid)
							.uuid()
							.not_null()
							.unique_key(),
					)
					.col(ColumnDef::new(SpaceGroups::SpaceId).integer().not_null())
					.col(ColumnDef::new(SpaceGroups::Name).string().not_null())
					.col(ColumnDef::new(SpaceGroups::GroupType).string().not_null())
					.col(
						ColumnDef::new(SpaceGroups::IsCollapsed)
							.boolean()
							.not_null()
							.default(false),
					)
					.col(
						ColumnDef::new(SpaceGroups::Order)
							.integer()
							.not_null()
							.default(0),
					)
					.col(
						ColumnDef::new(SpaceGroups::CreatedAt)
							.timestamp()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk_space_group_space")
							.from(SpaceGroups::Table, SpaceGroups::SpaceId)
							.to(Spaces::Table, Spaces::Id)
							.on_delete(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await?;

		// Create space_items table
		manager
			.create_table(
				Table::create()
					.table(SpaceItems::Table)
					.if_not_exists()
					.col(
						ColumnDef::new(SpaceItems::Id)
							.integer()
							.not_null()
							.auto_increment()
							.primary_key(),
					)
					.col(
						ColumnDef::new(SpaceItems::Uuid)
							.uuid()
							.not_null()
							.unique_key(),
					)
					.col(ColumnDef::new(SpaceItems::SpaceId).integer().not_null())
					.col(
						ColumnDef::new(SpaceItems::GroupId).integer().null(), // Nullable - None = space-level item
					)
					.col(ColumnDef::new(SpaceItems::ItemType).string().not_null())
					.col(
						ColumnDef::new(SpaceItems::Order)
							.integer()
							.not_null()
							.default(0),
					)
					.col(
						ColumnDef::new(SpaceItems::CreatedAt)
							.timestamp()
							.not_null()
							.default(Expr::current_timestamp()),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk_space_item_space")
							.from(SpaceItems::Table, SpaceItems::SpaceId)
							.to(Spaces::Table, Spaces::Id)
							.on_delete(ForeignKeyAction::Cascade),
					)
					.foreign_key(
						ForeignKey::create()
							.name("fk_space_item_group")
							.from(SpaceItems::Table, SpaceItems::GroupId)
							.to(SpaceGroups::Table, SpaceGroups::Id)
							.on_delete(ForeignKeyAction::Cascade),
					)
					.to_owned(),
			)
			.await?;

		// Create index for better query performance
		manager
			.create_index(
				Index::create()
					.name("idx_spaces_order")
					.table(Spaces::Table)
					.col(Spaces::Order)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("idx_space_groups_space_id")
					.table(SpaceGroups::Table)
					.col(SpaceGroups::SpaceId)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("idx_space_groups_order")
					.table(SpaceGroups::Table)
					.col(SpaceGroups::Order)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("idx_space_items_space_id")
					.table(SpaceItems::Table)
					.col(SpaceItems::SpaceId)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("idx_space_items_group_id")
					.table(SpaceItems::Table)
					.col(SpaceItems::GroupId)
					.to_owned(),
			)
			.await?;

		manager
			.create_index(
				Index::create()
					.name("idx_space_items_order")
					.table(SpaceItems::Table)
					.col(SpaceItems::Order)
					.to_owned(),
			)
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Drop tables in reverse order (respecting foreign keys)
		manager
			.drop_table(Table::drop().table(SpaceItems::Table).to_owned())
			.await?;

		manager
			.drop_table(Table::drop().table(SpaceGroups::Table).to_owned())
			.await?;

		manager
			.drop_table(Table::drop().table(Spaces::Table).to_owned())
			.await?;

		Ok(())
	}
}

#[derive(Iden)]
enum Spaces {
	Table,
	Id,
	Uuid,
	Name,
	Icon,
	Color,
	Order,
	CreatedAt,
	UpdatedAt,
}

#[derive(Iden)]
enum SpaceGroups {
	Table,
	Id,
	Uuid,
	SpaceId,
	Name,
	GroupType,
	IsCollapsed,
	Order,
	CreatedAt,
}

#[derive(Iden)]
enum SpaceItems {
	Table,
	Id,
	Uuid,
	SpaceId,
	GroupId,
	ItemType,
	Order,
	CreatedAt,
}
