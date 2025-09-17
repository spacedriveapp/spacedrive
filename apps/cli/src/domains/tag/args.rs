use clap::Args;
use uuid::Uuid;

use sd_core::ops::tags::{
	apply::input::ApplyTagsInput, create::input::CreateTagInput, search::input::SearchTagsInput,
};

#[derive(Args, Debug)]
pub struct TagCreateArgs {
	/// Canonical name for the tag
	pub name: String,
	/// Optional namespace
	#[arg(long)]
	pub namespace: Option<String>,
}

impl From<TagCreateArgs> for CreateTagInput {
	fn from(args: TagCreateArgs) -> Self {
		let mut input = CreateTagInput::simple(args.name);
		input.namespace = args.namespace;
		input
	}
}

#[derive(Args, Debug)]
pub struct TagApplyArgs {
	/// Entry IDs to tag (space-separated)
	#[arg(required = true)]
	pub entries: Vec<i32>,
	/// Tag IDs to apply (space-separated UUIDs)
	#[arg(long, required = true)]
	pub tags: Vec<Uuid>,
}

impl From<TagApplyArgs> for ApplyTagsInput {
	fn from(args: TagApplyArgs) -> Self {
		ApplyTagsInput::user_tags(args.entries, args.tags)
	}
}

#[derive(Args, Debug)]
pub struct TagSearchArgs {
	/// Query text
	pub query: String,
	/// Optional namespace
	#[arg(long)]
	pub namespace: Option<String>,
	/// Include archived tags
	#[arg(long)]
	pub include_archived: bool,
	/// Limit number of results
	#[arg(long)]
	pub limit: Option<usize>,
}

impl From<TagSearchArgs> for SearchTagsInput {
	fn from(args: TagSearchArgs) -> Self {
		SearchTagsInput {
			query: args.query,
			namespace: args.namespace,
			tag_type: None,
			include_archived: Some(args.include_archived),
			limit: args.limit.or(Some(50)),
			resolve_ambiguous: Some(false),
			context_tag_ids: None,
		}
	}
}
