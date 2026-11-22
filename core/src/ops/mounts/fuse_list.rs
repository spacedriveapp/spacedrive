//! The FUSE mounts the daemon holds, and whether it can make one.

use crate::{
	context::CoreContext,
	infra::query::{CoreQuery, QueryResult},
	service::mounts::fuse::{self, FuseMountInfo},
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type, Default)]
pub struct FuseListInput {}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FuseListOutput {
	/// This daemon was built with FUSE support and runs on Linux.
	pub supported: bool,
	/// The daemon's config allows mounts.
	pub enabled: bool,
	pub mounts: Vec<FuseMountInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FuseListQuery {
	#[allow(dead_code)]
	input: FuseListInput,
}

impl CoreQuery for FuseListQuery {
	type Input = FuseListInput;
	type Output = FuseListOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		_session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		Ok(FuseListOutput {
			supported: fuse::supported(),
			enabled: fuse::enabled(&context).await,
			mounts: fuse::list().await,
		})
	}
}

crate::register_core_query!(FuseListQuery, "mounts.fuse_list");
