use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "sidecar_availability")]
pub struct Model {
	#[sea_orm(primary_key)]
	pub id: i32,

	pub content_uuid: Uuid,

	pub kind: String,

	pub variant: String,

	pub device_uuid: Uuid,

	pub has: bool,

	pub size: Option<i64>,

	pub checksum: Option<String>,

	pub last_seen_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
	#[sea_orm(
		belongs_to = "super::content_identity::Entity",
		from = "Column::ContentUuid",
		to = "super::content_identity::Column::Uuid"
	)]
	ContentIdentity,

	#[sea_orm(
		belongs_to = "super::device::Entity",
		from = "Column::DeviceUuid",
		to = "super::device::Column::Uuid"
	)]
	Device,
}

impl Related<super::content_identity::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::ContentIdentity.def()
	}
}

impl Related<super::device::Entity> for Entity {
	fn to() -> RelationDef {
		Relation::Device.def()
	}
}

impl ActiveModelBehavior for ActiveModel {}

impl Entity {
	/// Update or insert availability record
	pub async fn update_or_insert(
		db: &DatabaseConnection,
		content_uuid: &Uuid,
		kind: &str,
		variant: &str,
		device_uuid: &Uuid,
		has: bool,
	) -> Result<Model, DbErr> {
		use sea_orm::{ActiveValue::Set, QueryFilter};

		let now = Utc::now();

		// Try to find existing record
		let existing = Self::find()
			.filter(Column::ContentUuid.eq(*content_uuid))
			.filter(Column::Kind.eq(kind))
			.filter(Column::Variant.eq(variant))
			.filter(Column::DeviceUuid.eq(*device_uuid))
			.one(db)
			.await?;

		if let Some(record) = existing {
			// Update existing
			let mut active: ActiveModel = record.into();
			active.has = Set(has);
			active.last_seen_at = Set(now);
			active.update(db).await
		} else {
			// Insert new
			let new_record = ActiveModel {
				content_uuid: Set(*content_uuid),
				kind: Set(kind.to_string()),
				variant: Set(variant.to_string()),
				device_uuid: Set(*device_uuid),
				has: Set(has),
				last_seen_at: Set(now),
				..Default::default()
			};
			new_record.insert(db).await
		}
	}
}
