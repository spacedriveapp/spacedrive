//! What the host's filesystem client actually asks a mount for.
//!
//! Experiments 2 and 5 in `docs/core/design/mounts.md`: the case for a
//! native mount module rests on a loopback client's read pattern being
//! worse than one we choose ourselves, and this is how that stops being an
//! argument and becomes a number.

use crate::{
	context::CoreContext,
	infra::query::{CoreQuery, QueryResult},
	service::mounts::trace,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type, Default)]
pub struct MountsReadTraceInput {}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ReadSizeBucket {
	pub range: String,
	pub reads: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FrontendReads {
	pub frontend: String,
	pub reads: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MountsReadTrace {
	/// Whether the recorder is on. Off means these numbers are from an
	/// earlier session, or empty.
	pub recording: bool,
	pub reads: u64,
	pub bytes: u64,
	pub files: u64,
	pub span_ms: u64,

	/// The read size the client asks for most often. The number the native
	/// mount decision turns on: a client issuing small reads leaves
	/// throughput on the table that a module choosing its own size keeps.
	pub common_read_bytes: u64,
	pub min_read_bytes: u64,
	pub max_read_bytes: u64,
	pub mean_read_bytes: u64,

	/// Reads continuing exactly where the last one for that file ended.
	pub sequential: u64,
	/// Reads jumping elsewhere in the file.
	pub seeks: u64,
	/// Reads overlapping bytes the client already had — waste a native
	/// module would not produce.
	pub rereads: u64,

	pub p50_micros: u64,
	pub p95_micros: u64,
	pub max_micros: u64,

	pub size_buckets: Vec<ReadSizeBucket>,
	pub by_frontend: Vec<FrontendReads>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MountsReadTraceQuery {
	#[allow(dead_code)]
	input: MountsReadTraceInput,
}

impl CoreQuery for MountsReadTraceQuery {
	type Input = MountsReadTraceInput;
	type Output = MountsReadTrace;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		_context: Arc<CoreContext>,
		_session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let s = trace::summary();
		Ok(MountsReadTrace {
			recording: s.enabled,
			reads: s.reads,
			bytes: s.bytes,
			files: s.files,
			span_ms: s.span_ms,
			common_read_bytes: s.common_read_bytes,
			min_read_bytes: if s.reads == 0 { 0 } else { s.min_read_bytes },
			max_read_bytes: s.max_read_bytes,
			mean_read_bytes: s.mean_read_bytes,
			sequential: s.sequential,
			seeks: s.seeks,
			rereads: s.rereads,
			p50_micros: s.p50_micros,
			p95_micros: s.p95_micros,
			max_micros: s.max_micros,
			size_buckets: s
				.size_buckets
				.into_iter()
				.map(|(range, reads)| ReadSizeBucket { range, reads })
				.collect(),
			by_frontend: s
				.by_frontend
				.into_iter()
				.map(|(frontend, reads)| FrontendReads { frontend, reads })
				.collect(),
		})
	}
}

crate::register_core_query!(MountsReadTraceQuery, "mounts.read_trace");
