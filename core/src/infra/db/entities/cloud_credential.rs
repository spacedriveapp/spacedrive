//! Cloud credential entity for encrypted storage of cloud service credentials

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "cloud_credentials")]
pub struct Model {
	#[sea_orm(primary_key)]
	pub id: i32,

	#[sea_orm(unique, indexed)]
	pub volume_fingerprint: String,

	pub encrypted_credential: Vec<u8>,

	pub service_type: String,

	pub created_at: DateTimeUtc,
	pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
