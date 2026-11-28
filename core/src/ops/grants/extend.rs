//! Give an active grant a new expiry.
//!
//! The new expiry is measured from now, not from the old one, so no sequence
//! of extensions takes a grant more than 12 hours past the moment it was last
//! extended, 30 days for a standing grant. Revoked and expired grants cannot
//! be extended: an expired grant has already told its holder to stop, and
//! reviving it would undo that. A signed token states its own expiry, so a
//! signed grant cannot be extended either; the owner creates a new one.

use std::sync::Arc;

use chrono::Utc;
use sd_store::GrantRow;
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
		// The space's record is read, or recreated for a grant made before
		// grants were asserted, before the local expiry moves, so a grant the
		// space cannot record is never left half extended.
		let assertion = match row.space_id {
			Some(space) => Some(
				space_assertion(&context, &library, &row, space)
					.await
					.map_err(|e| {
						ActionError::Internal(format!(
							"grant {id} cannot be extended in its space: {e}"
						))
					})?,
			),
			None => None,
		};
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
		if let Some((store, mut assertion)) = assertion {
			let written = match crate::ops::spaces::store::stamp(&context) {
				Ok(stamp) => {
					assertion.expires_at = expires_at.to_rfc3339();
					assertion.set_stamp(&stamp);
					store
						.db()
						.put_grant(&assertion)
						.await
						.map_err(|e| e.to_string())
				}
				Err(e) => Err(e),
			};
			written.map_err(|e| {
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

/// The space store holding a grant and the grant's assertion in it.
///
/// A grant created before grants were recorded in their space has a
/// `space_id` but no assertion; it is rebuilt here from the local row.
async fn space_assertion(
	context: &CoreContext,
	library: &Library,
	row: &agent_grant::Model,
	space: Uuid,
) -> Result<(Arc<crate::ops::indexing::store::SourceStore>, GrantRow), String> {
	let store = super::create::space_store(context, row.source_id)
		.await
		.map_err(|e| e.to_string())?;
	if let Some(assertion) = store
		.db()
		.grant(row.uuid)
		.await
		.map_err(|e| e.to_string())?
	{
		return Ok((store, assertion));
	}
	let at = crate::ops::spaces::events::space_of(
		context,
		std::path::Path::new(&row.subtree),
		Some(library.id()),
	)
	.await
	.filter(|at| at.space_id == space)
	.ok_or_else(|| format!("the grant's folder is no longer in space {space}"))?;
	let issuer = Uuid::parse_str(&row.created_by)
		.map_err(|e| format!("the grant's issuer is not a device id: {e}"))?;
	let stamp = crate::ops::spaces::store::stamp(context)?;
	Ok((
		store,
		GrantRow {
			uuid: row.uuid,
			space_uuid: space,
			actor_uuid: row.actor_id,
			kind: row.kind.clone(),
			token_format: row.token_format.clone(),
			subtree: at.path,
			caps: row.caps.clone(),
			label: row.label.clone(),
			issuer,
			epoch: row.epoch,
			created_at: row.created_at.to_rfc3339(),
			expires_at: row.expires_at.to_rfc3339(),
			updated_hlc: stamp.hlc,
			origin_device: stamp.device_uuid,
		},
	))
}

crate::register_library_action!(ExtendGrantAction, "grants.extend");
