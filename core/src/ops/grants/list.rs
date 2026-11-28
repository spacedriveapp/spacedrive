//! Every grant in the library, for the owner.

use std::sync::Arc;

use sea_orm::{EntityTrait, QueryOrder};
use serde::{Deserialize, Serialize};
use specta::Type;

use super::{audit, store, GrantInfo, GrantStatus};
use crate::{
	context::CoreContext,
	infra::{
		db::entities::agent_grant,
		query::{LibraryQuery, QueryError, QueryResult},
	},
};

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
pub struct ListGrantsInput {
	/// Include expired and revoked grants. Only active ones otherwise.
	#[serde(default)]
	pub include_inactive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ListGrantsOutput {
	/// Newest first.
	pub grants: Vec<GrantInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListGrantsQuery {
	input: ListGrantsInput,
}

impl LibraryQuery for ListGrantsQuery {
	type Input = ListGrantsInput;
	type Output = ListGrantsOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let library = store::session_library(&context, &session).await?;
		let db = library.db().conn();
		let internal = |e: sea_orm::DbErr| QueryError::Internal(format!("list grants: {e}"));
		let rows = agent_grant::Entity::find()
			.order_by_desc(agent_grant::Column::Id)
			.all(db)
			.await
			.map_err(internal)?;
		let mut usage = audit::usage(db).await.map_err(internal)?;
		let grants = rows
			.into_iter()
			.map(|row| {
				let used = usage.remove(&row.uuid).unwrap_or_default();
				store::info(row, used)
			})
			.filter(|grant| self.input.include_inactive || grant.status == GrantStatus::Active)
			.collect();
		Ok(ListGrantsOutput { grants })
	}
}

crate::register_library_query!(ListGrantsQuery, "grants.list");
