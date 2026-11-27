//! Give an active grant a new expiry.
//!
//! The new expiry is measured from now, not from the old one, so no sequence
//! of extensions takes a grant more than 12 hours past the moment it was last
//! extended, 30 days for a standing grant. Revoked and expired grants cannot
//! be extended: an expired grant has already told its holder to stop, and
//! reviving it would undo that. A signed token states its own expiry, so a
//! signed grant cannot be extended either; the owner creates a new one.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use sea_orm::{sea_query::Expr, ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use specta::Type;
use tracing::info;
use uuid::Uuid;

use super::{store, ttl_for, GrantInfo, GrantKind, GrantStatus, TokenFormat};
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
	/// The new lifetime in seconds from now: 5 minutes to 12 hours for a task
	/// grant, 1 hour by default; up to 30 days for a standing grant, 30 days
	/// by default.
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
		ttl_for(GrantKind::Standing, input.ttl_seconds)?;
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: Arc<Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let db = library.db().conn();
		let id = self.input.id;
		let now = Utc::now();
		let row = store::by_id(db, id)
			.await
			.map_err(|e| ActionError::Internal(e.to_string()))?
			.ok_or_else(|| ActionError::InvalidInput(format!("no grant {id}")))?;
		if TokenFormat::parse(&row.token_format) == TokenFormat::Signed {
			return Err(ActionError::InvalidInput(format!(
				"grant {id} has a signed token, which carries its own expiry; create a new grant"
			)));
		}
		let lifetime = ttl_for(GrantKind::parse(&row.kind), self.input.ttl_seconds)
			.map_err(ActionError::InvalidInput)?;
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
		if let Some(space) = row.space_id {
			extend_assertion(&context, row.source_id, space, id, expires_at)
				.await
				.map_err(|e| {
					ActionError::Internal(format!(
						"grant {id} is extended on this device, but not in its space: {e}"
					))
				})?;
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

/// Carry a new expiry to the grant's assertion, so other nodes holding the
/// space's store see the extension.
async fn extend_assertion(
	context: &CoreContext,
	source_id: Uuid,
	space: Uuid,
	id: Uuid,
	expires_at: DateTime<Utc>,
) -> Result<(), String> {
	let store = super::create::space_store(context, source_id)
		.await
		.map_err(|e| e.to_string())?;
	let Some(mut row) = store.db().grant(id).await.map_err(|e| e.to_string())? else {
		return Err(format!("the space {space} holds no record of the grant"));
	};
	row.expires_at = expires_at.to_rfc3339();
	row.set_stamp(&crate::ops::spaces::store::stamp(context)?);
	store
		.db()
		.put_grant(&row)
		.await
		.map_err(|e| e.to_string())?;
	Ok(())
}

crate::register_library_action!(ExtendGrantAction, "grants.extend");
