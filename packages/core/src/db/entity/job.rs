use chrono::NaiveDateTime;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

// -------------------------------------
// Entity: Job
//
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, DeriveEntityModel, TS)]
#[sea_orm(table_name = "jobs")]
#[serde(rename = "Job")]
#[ts(export)]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: u32,
    // the client that will perform this task
    pub client_id: u32,
    // what kind of task is this
    pub action: Action,
    // status
    pub status: Status,
    pub percentage_complete: u32,
    pub task_count: u32,
    pub completed_task_count: u32,

    #[ts(type = "string")]
    pub date_created: Option<NaiveDateTime>,
    #[ts(type = "string")]
    pub date_modified: Option<NaiveDateTime>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, EnumIter, DeriveActiveEnum, TS)]
#[sea_orm(rs_type = "u32", db_type = "Integer")]
#[ts(export)]
pub enum Action {
    #[sea_orm(num_value = 0)]
    Scan,
    #[sea_orm(num_value = 1)]
    Encrypt,
    #[sea_orm(num_value = 2)]
    Upload,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, EnumIter, DeriveActiveEnum, TS)]
#[sea_orm(rs_type = "u32", db_type = "Integer")]
#[ts(export)]
pub enum Status {
    #[sea_orm(num_value = 0)]
    Queued,
    #[sea_orm(num_value = 1)]
    InProgress,
    #[sea_orm(num_value = 2)]
    Cancelled,
    #[sea_orm(num_value = 3)]
    Completed,
    #[sea_orm(num_value = 4)]
    Failed,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::client::Entity",
        from = "Column::ClientId",
        to = "super::client::Column::Id"
    )]
    Client,
}

impl ActiveModelBehavior for ActiveModel {}
