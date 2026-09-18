//! Definitions applied nowhere yet. A source adopts a definition the first
//! time an assertion lands there, and adoption deletes the staging row, so
//! this table only ever holds tags waiting for their first use.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "tag_staging")]
pub struct Model {
	#[sea_orm(primary_key)]
	pub id: i32,

	#[sea_orm(unique)]
	pub uuid: Uuid,

	/// Merge key: v5 of the folded path, matching `slug_for_path` in
	/// `sd_store::tags`.
	pub slug_id: Uuid,

	pub path: String,
	pub color: Option<String>,
	pub icon: Option<String>,

	/// Sortable HLC text; carried into stores on adoption.
	pub updated_hlc: String,
	pub origin_device: Uuid,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
