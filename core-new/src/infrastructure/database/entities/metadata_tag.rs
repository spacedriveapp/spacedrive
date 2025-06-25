//! UserMetadataTag junction entity for hierarchical metadata tagging

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "user_metadata_tags")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub user_metadata_id: i32,
    #[sea_orm(primary_key)]
    pub tag_uuid: Uuid,
    pub created_at: DateTimeUtc,
    pub device_uuid: Uuid,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user_metadata::Entity",
        from = "Column::UserMetadataId",
        to = "super::user_metadata::Column::Id"
    )]
    UserMetadata,
    #[sea_orm(
        belongs_to = "super::tag::Entity",
        from = "Column::TagUuid",
        to = "super::tag::Column::Uuid"
    )]
    Tag,
    #[sea_orm(
        belongs_to = "super::device::Entity",
        from = "Column::DeviceUuid",
        to = "super::device::Column::Uuid"
    )]
    Device,
}

impl Related<super::user_metadata::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserMetadata.def()
    }
}

impl Related<super::tag::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Tag.def()
    }
}

impl Related<super::device::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Device.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}