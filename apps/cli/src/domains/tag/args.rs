use clap::Args;
use uuid::Uuid;

use sd_core::ops::tags::{
	apply::input::{ApplyTagsInput, TagTargets},
	create::input::CreateTagInput,
	delete::input::DeleteTagInput,
	search::input::SearchTagsInput,
	unapply::input::UnapplyTagsInput,
};

#[derive(Args, Debug)]
pub struct TagCreateArgs {
	/// Full tag path, like "Work/Clients/Acme"
	pub path: String,
	/// Hex color, like "#ff5500"
	#[arg(long)]
	pub color: Option<String>,
	/// Icon name
	#[arg(long)]
	pub icon: Option<String>,
}

impl From<TagCreateArgs> for CreateTagInput {
	fn from(args: TagCreateArgs) -> Self {
		CreateTagInput {
			path: args.path,
			color: args.color,
			icon: args.icon,
		}
	}
}

#[derive(Args, Debug)]
pub struct TagApplyArgs {
	/// File UUIDs to tag (space-separated)
	#[arg(required = true)]
	pub files: Vec<Uuid>,
	/// Tag IDs to apply (space-separated UUIDs)
	#[arg(long, required = true)]
	pub tags: Vec<Uuid>,
	/// Tag the bytes instead: the file UUIDs are content UUIDs, and the tag
	/// reaches every copy
	#[arg(long)]
	pub content: bool,
}

impl From<TagApplyArgs> for ApplyTagsInput {
	fn from(args: TagApplyArgs) -> Self {
		ApplyTagsInput {
			targets: if args.content {
				TagTargets::Content(args.files)
			} else {
				TagTargets::File(args.files)
			},
			tag_ids: args.tags,
		}
	}
}

#[derive(Args, Debug)]
pub struct TagUnapplyArgs {
	/// File UUIDs to untag (space-separated)
	#[arg(required = true)]
	pub files: Vec<Uuid>,
	/// Tag IDs to remove (space-separated UUIDs)
	#[arg(long, required = true)]
	pub tags: Vec<Uuid>,
	/// Untag the bytes instead: the file UUIDs are content UUIDs
	#[arg(long)]
	pub content: bool,
}

impl From<TagUnapplyArgs> for UnapplyTagsInput {
	fn from(args: TagUnapplyArgs) -> Self {
		UnapplyTagsInput {
			targets: if args.content {
				TagTargets::Content(args.files)
			} else {
				TagTargets::File(args.files)
			},
			tag_ids: args.tags,
		}
	}
}

#[derive(Args, Debug)]
pub struct TagDeleteArgs {
	/// The tag's UUID
	pub tag: Uuid,
}

impl From<TagDeleteArgs> for DeleteTagInput {
	fn from(args: TagDeleteArgs) -> Self {
		DeleteTagInput { tag_id: args.tag }
	}
}

#[derive(Args, Debug)]
pub struct TagSearchArgs {
	/// Query text; empty lists every tag
	#[arg(default_value = "")]
	pub query: String,
	/// Limit number of results
	#[arg(long)]
	pub limit: Option<u32>,
}

impl From<TagSearchArgs> for SearchTagsInput {
	fn from(args: TagSearchArgs) -> Self {
		SearchTagsInput {
			query: args.query,
			limit: args.limit,
		}
	}
}
