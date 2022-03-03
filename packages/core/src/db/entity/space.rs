use chrono::NaiveDateTime;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
// -------------------------------------
// Entity: Space
// Spaces are virtual directories that can be used to organize, and visualize, projects.
// They're sharable and can be made available on the web.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, DeriveEntityModel, Default, TS)]
#[sea_orm(table_name = "spaces")]
#[serde(rename = "Space")]
#[ts(export)]
// -------------------------------------
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: u32,
    pub name: String,
    pub calculated_size_in_bytes: Option<String>,
    pub calculated_file_count: Option<u32>,
    pub library_id: String,
    #[ts(type = "string")]
    pub date_created: Option<NaiveDateTime>,
    #[ts(type = "string")]
    pub date_modified: Option<NaiveDateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::library::Entity",
        from = "Column::LibraryId",
        to = "super::library::Column::Id"
    )]
    Library,
}

impl ActiveModelBehavior for ActiveModel {}
