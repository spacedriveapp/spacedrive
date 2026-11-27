//! `cli.status`: where `sd` points.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;

use super::CliInstallStatus;
use crate::{
	context::CoreContext,
	infra::query::{CoreQuery, QueryResult},
};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CliStatusInput {}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CliStatusQuery;

impl CoreQuery for CliStatusQuery {
	type Input = CliStatusInput;
	type Output = CliInstallStatus;

	fn from_input(_input: Self::Input) -> QueryResult<Self> {
		Ok(Self)
	}

	async fn execute(
		self,
		_context: Arc<CoreContext>,
		_session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		Ok(super::status().await)
	}
}

crate::register_core_query!(CliStatusQuery, "cli.status");
