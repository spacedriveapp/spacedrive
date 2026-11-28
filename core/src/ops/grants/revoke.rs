//! End a grant now.
//!
//! The revocation is set on the grant row and, for a grant in a space,
//! recorded as an assertion in the space's store. [`super::authorize`] reads
//! both on every request, so the holder's next request after this returns is
//! refused here, and on any other node as soon as the store reaches it.
//! Revoking twice keeps the first revocation time.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;
use tracing::info;
use uuid::Uuid;

use super::{store, GrantInfo};
use crate::{
	context::CoreContext,
	infra::action::{error::ActionError, LibraryAction},
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
		context: Arc<CoreContext>,
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
		store::revoke(db, id)
			.await
			.map_err(|e| ActionError::Internal(format!("revoke grant: {e}")))?;
		let row = store::by_id(db, id)
			.await
			.map_err(|e| ActionError::Internal(e.to_string()))?
			.ok_or_else(|| ActionError::Internal(format!("grant {id} vanished")))?;
		// The grant is already refused here. Revoking again retries the
		// assertion, so other nodes learn of it once the store is writable.
		store::publish_revocation(&context, &row)
			.await
			.map_err(|e| {
				ActionError::Internal(format!(
					"grant {id} is revoked on this device, but the revocation could not be recorded in its space: {e}"
				))
			})?;
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
