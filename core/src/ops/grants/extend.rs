//! Give an active grant a new expiry.
//!
//! The new expiry is measured from now, not from the old one, so no sequence
//! of extensions takes a grant more than 12 hours past the moment it was last
//! extended. Revoked and expired grants cannot be extended: an expired grant
//! has already told its holder to stop, and reviving it would undo that.

use std::sync::Arc;

use chrono::Utc;
use sea_orm::{sea_query::Expr, ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use specta::Type;
use tracing::info;
use uuid::Uuid;

use super::{store, ttl, GrantInfo, GrantStatus};
use crate::{
	context::CoreContext,
	infra::{
		action::{error::ActionError, LibraryAction},
		db::entities::agent_grant,
	},
	library::Library,
};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ExtendGrantInput {
	pub id: Uuid,
	/// The new lifetime in seconds from now, from 5 minutes to 12 hours.
	/// Defaults to 1 hour.
	#[serde(default)]
	pub ttl_seconds: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ExtendGrantOutput {
	pub grant: GrantInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtendGrantAction {
	input: ExtendGrantInput,
}

impl LibraryAction for ExtendGrantAction {
	type Input = ExtendGrantInput;
	type Output = ExtendGrantOutput;

	fn from_input(input: ExtendGrantInput) -> Result<Self, String> {
		ttl(input.ttl_seconds)?;
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: Arc<Library>,
		_context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let lifetime = ttl(self.input.ttl_seconds).map_err(ActionError::InvalidInput)?;
		let db = library.db().conn();
		let id = self.input.id;
		let now = Utc::now();
		let row = store::by_id(db, id)
			.await
			.map_err(|e| ActionError::Internal(e.to_string()))?
			.ok_or_else(|| ActionError::InvalidInput(format!("no grant {id}")))?;
		match store::status(&row, now) {
			GrantStatus::Active => {}
			GrantStatus::Revoked => {
				return Err(ActionError::InvalidInput(format!(
					"grant {id} was revoked; create a new one"
				)))
			}
			GrantStatus::Expired => {
				return Err(ActionError::InvalidInput(format!(
					"grant {id} has expired; create a new one"
				)))
			}
		}

		let expires_at = now + lifetime;
		// The revoked filter closes the window between the status read and the
		// write, so a concurrent revoke is never overwritten into a live grant.
		let updated = agent_grant::Entity::update_many()
			.col_expr(agent_grant::Column::ExpiresAt, Expr::value(expires_at))
			.filter(agent_grant::Column::Uuid.eq(id))
			.filter(agent_grant::Column::RevokedAt.is_null())
			.exec(db)
			.await
			.map_err(|e| ActionError::Internal(format!("extend grant: {e}")))?;
		if updated.rows_affected == 0 {
			return Err(ActionError::InvalidInput(format!(
				"grant {id} was revoked; create a new one"
			)));
		}
		info!(grant = %id, %expires_at, "grant extended");
		let grant = store::info_by_id(db, id)
			.await
			.map_err(|e| ActionError::Internal(e.to_string()))?;
		Ok(ExtendGrantOutput { grant })
	}

	fn action_kind(&self) -> &'static str {
		"grants.extend"
	}
}

crate::register_library_action!(ExtendGrantAction, "grants.extend");
