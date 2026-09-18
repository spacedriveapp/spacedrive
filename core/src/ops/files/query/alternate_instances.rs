//! Every copy of one file's bytes, wherever this machine keeps them.
//!
//! Answers from content identity rather than from a path or a size, so a copy
//! renamed on the way to its second home is still the same file. The identity
//! is derived from the bytes, so the answer can span drives: each source is
//! asked about the same content uuid and they agree without comparing notes.
//!
//! A record whose bytes have not been hashed yet has no identity and therefore
//! no alternates, which is the honest answer rather than a guess from size.

use crate::{
	context::CoreContext,
	domain::{addressing::SdPath, file::File},
	infra::query::{LibraryQuery, QueryResult},
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use uuid::Uuid;

/// Input for alternate instances query
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct AlternateInstancesInput {
	/// The entry UUID to find alternates for
	pub entry_uuid: Uuid,
}

/// Output containing alternate instances
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct AlternateInstancesOutput {
	/// All instances of this file (including the original)
	pub instances: Vec<File>,
	/// Total number of instances found
	pub total_count: u32,
}

/// Query to get alternate instances of a file
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct AlternateInstancesQuery {
	pub input: AlternateInstancesInput,
}

impl AlternateInstancesQuery {
	pub fn new(entry_uuid: Uuid) -> Self {
		Self {
			input: AlternateInstancesInput { entry_uuid },
		}
	}
}

impl LibraryQuery for AlternateInstancesQuery {
	type Input = AlternateInstancesInput;
	type Output = AlternateInstancesOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		_session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let cache = context.volume_index();

		let Some(content) = cache.content_of(self.input.entry_uuid).await else {
			return Ok(AlternateInstancesOutput {
				instances: Vec::new(),
				total_count: 0,
			});
		};

		let mut instances = Vec::new();
		for copy in cache.copies_of_content(content).await {
			let index = cache.resolve_index(&copy.path);
			let mut index = index.write().await;
			let Some(metadata) = index.get_entry_ref(&copy.path) else {
				continue;
			};
			let kind = index.get_content_kind(&copy.path);
			drop(index);

			let mut file = File::from_arena(
				copy.record_uuid,
				&metadata,
				SdPath::local(copy.path.clone()),
			);
			file.content_kind = kind;
			instances.push(file);
		}

		instances.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

		Ok(AlternateInstancesOutput {
			total_count: instances.len() as u32,
			instances,
		})
	}
}

crate::register_library_query!(AlternateInstancesQuery, "files.alternate_instances");
