//! A tag's ancestors, derived from its path.
//!
//! Hierarchy travels as a path rather than as parent pointers, so ancestry
//! is the chain of path prefixes. Only prefixes that exist as definitions
//! are returned: a bare intermediate path is implicit structure, and there
//! is nothing to render for it beyond the segments the child already shows.

use crate::{
	context::CoreContext,
	domain::Tag,
	infra::query::{LibraryQuery, QueryError, QueryResult},
	ops::tags::definitions,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetTagAncestorsInput {
	pub tag_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetTagAncestorsOutput {
	/// Root first, immediate parent last.
	pub tags: Vec<Tag>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTagAncestorsQuery {
	input: GetTagAncestorsInput,
}

impl LibraryQuery for GetTagAncestorsQuery {
	type Input = GetTagAncestorsInput;
	type Output = GetTagAncestorsOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let library_id = session
			.current_library_id
			.ok_or_else(|| QueryError::Internal("No library in session".to_string()))?;
		let library = context
			.libraries()
			.await
			.get_library(library_id)
			.await
			.ok_or_else(|| QueryError::Internal("Library not found".to_string()))?;
		let cache = context.volume_index();

		let all = definitions::all(&library, &cache).await;
		let Some(target) = all.iter().find(|d| d.uuid == self.input.tag_id) else {
			return Ok(GetTagAncestorsOutput { tags: Vec::new() });
		};

		let segments: Vec<&str> = target.path.split('/').collect();
		let mut tags = Vec::new();
		for depth in 1..segments.len() {
			let prefix = segments[..depth].join("/");
			if let Some(ancestor) = all.iter().find(|d| d.path == prefix) {
				tags.push(Tag::from_definition(ancestor));
			}
		}

		Ok(GetTagAncestorsOutput { tags })
	}
}

crate::register_library_query!(GetTagAncestorsQuery, "tags.ancestors");
