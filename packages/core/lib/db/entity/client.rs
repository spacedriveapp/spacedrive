use chrono::NaiveDateTime;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

// -------------------------------------
// Entity: Client
// Represents an instance of a Spacedrive client
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, DeriveEntityModel, TS)]
#[sea_orm(table_name = "clients")]
#[serde(rename = "Client")]
#[ts(export)]
// -------------------------------------
pub struct Model {
    // identity
    #[sea_orm(primary_key)]
    pub id: u32,
    pub name: String,
    pub platform: Platform,
    pub online: bool,
    #[ts(type = "string")]
    pub last_seen: Option<NaiveDateTime>,
    pub timezone: Option<String>,
    #[ts(type = "string")]
    pub date_created: Option<NaiveDateTime>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, EnumIter, DeriveActiveEnum, TS)]
#[sea_orm(rs_type = "i32", db_type = "Integer")]
#[ts(export)]
pub enum Platform {
    #[sea_orm(num_value = 0)]
    Unknown,
    #[sea_orm(num_value = 1)]
    MacOS,
    #[sea_orm(num_value = 2)]
    Windows,
    #[sea_orm(num_value = 3)]
    Linux,
    #[sea_orm(num_value = 4)]
    IOS,
    #[sea_orm(num_value = 5)]
    Android,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
