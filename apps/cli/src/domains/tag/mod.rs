mod args;

use anyhow::Result;
use clap::Subcommand;

use crate::context::Context;
use crate::util::prelude::*;

use sd_core::ops::tags::{
	apply::output::ApplyTagsOutput, create::output::CreateTagOutput,
	search::output::SearchTagsOutput, search::query::SearchTagsQuery,
};

use self::args::*;

#[derive(Subcommand, Debug)]
pub enum TagCmd {
	/// Create a new tag
	Create(TagCreateArgs),
	/// Apply one or more tags to entries
	Apply(TagApplyArgs),
	/// Search for tags
	Search(TagSearchArgs),
}

pub async fn run(ctx: &Context, cmd: TagCmd) -> Result<()> {
	match cmd {
		TagCmd::Create(args) => {
			let input: sd_core::ops::tags::create::input::CreateTagInput = args.into();
			let out: CreateTagOutput = execute_action!(ctx, input);
			print_output!(ctx, &out, |o: &CreateTagOutput| {
				println!("{} (id: {})", o.canonical_name, o.tag_id);
			});
		}
		TagCmd::Apply(args) => {
			let input: sd_core::ops::tags::apply::input::ApplyTagsInput = args.into();
			let out: ApplyTagsOutput = execute_action!(ctx, input);
			print_output!(ctx, &out, |o: &ApplyTagsOutput| {
				println!(
					"Applied {} tag(s) to {} entries",
					o.tags_applied, o.entries_affected
				);
			});
		}
		TagCmd::Search(args) => {
			let input: sd_core::ops::tags::search::input::SearchTagsInput = args.into();
			let out: SearchTagsOutput = execute_query!(ctx, input);
			print_output!(ctx, &out, |o: &SearchTagsOutput| {
				if o.tags.is_empty() {
					println!("No tags found");
					return;
				}
				for r in &o.tags {
					println!("{} {}", r.tag.id, r.tag.canonical_name);
				}
			});
		}
	}
	Ok(())
}
