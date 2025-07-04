//! Audit log entity for tracking user actions

use sea_orm::entity::prelude::*;
use sea_orm::Set;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "audit_log")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    
    #[sea_orm(unique)]
    pub uuid: Uuid,

    #[sea_orm(indexed)]
    pub action_type: String,

    #[sea_orm(indexed)]
    pub actor_device_id: Uuid,

    #[sea_orm(column_type = "Json")]
    pub targets: Json,

    #[sea_orm(indexed)]
    pub status: ActionStatus,

    #[sea_orm(indexed, nullable)]
    pub job_id: Option<Uuid>,

    pub created_at: DateTimeUtc,
    pub completed_at: Option<DateTimeUtc>,
    
    pub error_message: Option<String>,
    
    #[sea_orm(column_type = "Json", nullable)]
    pub result_payload: Option<Json>,
}

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum ActionStatus {
    #[sea_orm(string_value = "in_progress")]
    InProgress,
    #[sea_orm(string_value = "completed")]
    Completed,
    #[sea_orm(string_value = "failed")]
    Failed,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {
    fn new() -> Self {
        Self {
            uuid: Set(Uuid::new_v4()),
            created_at: Set(chrono::Utc::now()),
            ..ActiveModelTrait::default()
        }
    }
}