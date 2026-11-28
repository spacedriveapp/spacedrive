//! # List NFS mounts
//!
//! The NFS mounts the daemon holds, and whether it can make one.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
	context::CoreContext,
	infra::query::{CoreQuery, QueryResult},
	service::mounts::nfs::{self, NfsMountInfo},
};

#[derive(Debug, Clone, Serialize, Deserialize, Type, Default)]
pub struct NfsListInput {}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct NfsListOutput {
	/// This daemon was built with NFS support and runs on macOS or Linux.
	pub supported: bool,
	/// The daemon's config allows mounts.
	pub enabled: bool,
	pub mounts: Vec<NfsMountInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct NfsListQuery {
	#[allow(dead_code)]
	input: NfsListInput,
}

impl CoreQuery for NfsListQuery {
	type Input = NfsListInput;
	type Output = NfsListOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		_session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		Ok(NfsListOutput {
			supported: nfs::supported(),
			enabled: nfs::enabled(&context).await,
			mounts: nfs::list().await,
		})
	}
}

crate::register_core_query!(NfsListQuery, "mounts.nfs_list");
