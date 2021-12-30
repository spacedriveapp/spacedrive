use chrono::NaiveDateTime;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

// -------------------------------------
// Entity: LocationPaths
//
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, DeriveEntityModel, TS)]
#[sea_orm(table_name = "location_paths")]
#[serde(rename = "LocationPaths")]
#[ts(export)]
// -------------------------------------
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: u32,
    pub path: String,
    pub rule: PathRule,
    pub location_id: String,
    #[ts(type = "string")]
    pub date_created: Option<NaiveDateTime>,
    #[ts(type = "string")]
    pub date_modified: Option<NaiveDateTime>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, EnumIter, DeriveActiveEnum, TS)]
#[sea_orm(rs_type = "i32", db_type = "Integer")]
#[ts(export)]
pub enum PathRule {
    #[sea_orm(num_value = 0)]
    Exclude,
    #[sea_orm(num_value = 1)]
    NoWatch,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::locations::Entity",
        from = "Column::LocationId",
        to = "super::locations::Column::Id"
    )]
    Library,
}

impl ActiveModelBehavior for ActiveModel {}
