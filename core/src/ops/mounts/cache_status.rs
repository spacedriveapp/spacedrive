//! Query for the mount block cache: how much it holds and how often it
//! answers, so a working session can be characterised without a debugger.

use crate::{
	context::CoreContext,
	infra::query::{CoreQuery, QueryResult},
	service::mounts::cache,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type, Default)]
pub struct MountsCacheStatusInput {}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MountsCacheStatus {
	pub running: bool,
	/// Bytes and blocks resident in memory.
	pub l1_bytes: u64,
	pub l1_blocks: u64,
	/// Bytes and blocks on disk.
	pub l2_bytes: u64,
	pub l2_blocks: u64,
	pub max_bytes: u64,
	pub block_bytes: u64,
	pub l1_hits: u64,
	pub l2_hits: u64,
	/// Blocks that had to be fetched from the provider.
	pub misses: u64,
	/// Bytes pulled from providers, against bytes handed to readers — the
	/// ratio is what the cache is worth on this workload.
	pub fetched_bytes: u64,
	pub served_bytes: u64,
	pub evicted_blocks: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MountsCacheStatusQuery {
	#[allow(dead_code)]
	input: MountsCacheStatusInput,
}

impl CoreQuery for MountsCacheStatusQuery {
	type Input = MountsCacheStatusInput;
	type Output = MountsCacheStatus;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		_context: Arc<CoreContext>,
		_session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let Some(cache) = cache::cache() else {
			return Ok(MountsCacheStatus {
				running: false,
				l1_bytes: 0,
				l1_blocks: 0,
				l2_bytes: 0,
				l2_blocks: 0,
				max_bytes: 0,
				block_bytes: 0,
				l1_hits: 0,
				l2_hits: 0,
				misses: 0,
				fetched_bytes: 0,
				served_bytes: 0,
				evicted_blocks: 0,
			});
		};

		let snap = cache.snapshot();
		Ok(MountsCacheStatus {
			running: true,
			l1_bytes: snap.l1_bytes,
			l1_blocks: snap.l1_blocks,
			l2_bytes: snap.l2_bytes,
			l2_blocks: snap.l2_blocks,
			max_bytes: snap.max_bytes,
			block_bytes: snap.block_bytes,
			l1_hits: snap.l1_hits,
			l2_hits: snap.l2_hits,
			misses: snap.misses,
			fetched_bytes: snap.fetched_bytes,
			served_bytes: snap.served_bytes,
			evicted_blocks: snap.evicted_blocks,
		})
	}
}

crate::register_core_query!(MountsCacheStatusQuery, "mounts.cache_status");
