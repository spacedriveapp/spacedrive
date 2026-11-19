//! End a grant now.
//!
//! [`super::authorize`] reads the row on every request, so the holder's next
//! request after this returns is refused. Revoking twice keeps the first
//! revocation time.

use std::sync::Arc;

use chrono::Utc;
use sea_orm::{sea_query::Expr, ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use specta::Type;
use tracing::info;
use uuid::Uuid;

use super::{store, GrantInfo};
use crate::{
	context::CoreContext,
	infra::{
		action::{error::ActionError, LibraryAction},
		db::entities::agent_grant,
	},
	library::Library,
};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RevokeGrantInput {
	pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RevokeGrantOutput {
	pub grant: GrantInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevokeGrantAction {
	input: RevokeGrantInput,
}

impl LibraryAction for RevokeGrantAction {
	type Input = RevokeGrantInput;
	type Output = RevokeGrantOutput;

	fn from_input(input: RevokeGrantInput) -> Result<Self, String> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: Arc<Library>,
		_context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let db = library.db().conn();
		let id = self.input.id;
		if store::by_id(db, id)
			.await
			.map_err(|e| ActionError::Internal(e.to_string()))?
			.is_none()
		{
			return Err(ActionError::InvalidInput(format!("no grant {id}")));
		}
		agent_grant::Entity::update_many()
			.col_expr(agent_grant::Column::RevokedAt, Expr::value(Utc::now()))
			.filter(agent_grant::Column::Uuid.eq(id))
			.filter(agent_grant::Column::RevokedAt.is_null())
			.exec(db)
			.await
			.map_err(|e| ActionError::Internal(format!("revoke grant: {e}")))?;
		info!(grant = %id, "grant revoked");
		let grant = store::info_by_id(db, id)
			.await
			.map_err(|e| ActionError::Internal(e.to_string()))?;
		Ok(RevokeGrantOutput { grant })
	}

	fn action_kind(&self) -> &'static str {
		"grants.revoke"
	}
}

crate::register_library_action!(RevokeGrantAction, "grants.revoke");
