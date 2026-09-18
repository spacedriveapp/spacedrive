mod args;

use anyhow::Result;
use clap::Subcommand;

use crate::context::Context;
use crate::util::prelude::*;

use sd_core::ops::tags::{
	apply::output::ApplyTagsOutput, create::output::CreateTagOutput,
	delete::output::DeleteTagOutput, search::output::SearchTagsOutput,
	unapply::output::UnapplyTagsOutput,
};

use self::args::*;

#[derive(Subcommand, Debug)]
pub enum TagCmd {
	/// Create a tag by path, like "Work/Clients/Acme"
	Create(TagCreateArgs),
	/// Apply one or more tags to files
	Apply(TagApplyArgs),
	/// Remove one or more tags from files
	Unapply(TagUnapplyArgs),
	/// Delete a tag definition everywhere this daemon can write
	Delete(TagDeleteArgs),
	/// Search tags; an empty query lists all of them
	Search(TagSearchArgs),
}

pub async fn run(ctx: &Context, cmd: TagCmd) -> Result<()> {
	match cmd {
		TagCmd::Create(args) => {
			let input: sd_core::ops::tags::create::input::CreateTagInput = args.into();
			let out: CreateTagOutput = execute_action!(ctx, input);
			print_output!(ctx, &out, |o: &CreateTagOutput| {
				let verb = if o.created {
					"Created"
				} else {
					"Already exists:"
				};
				println!("{} {} (id: {})", verb, o.tag.path, o.tag.id);
			});
		}
		TagCmd::Apply(args) => {
			let input: sd_core::ops::tags::apply::input::ApplyTagsInput = args.into();
			let out: ApplyTagsOutput = execute_action!(ctx, input);
			print_output!(ctx, &out, |o: &ApplyTagsOutput| {
				println!("Tagged {} target(s)", o.targets_tagged);
				if o.targets_pending > 0 {
					println!(
						"{} target(s) pending delivery to their owner",
						o.targets_pending
					);
				}
				for warning in &o.warnings {
					println!("warning: {warning}");
				}
			});
		}
		TagCmd::Unapply(args) => {
			let input: sd_core::ops::tags::unapply::input::UnapplyTagsInput = args.into();
			let out: UnapplyTagsOutput = execute_action!(ctx, input);
			print_output!(ctx, &out, |o: &UnapplyTagsOutput| {
				println!("Untagged {} target(s)", o.targets_untagged);
				if o.targets_pending > 0 {
					println!(
						"{} removal(s) pending delivery to their owner",
						o.targets_pending
					);
				}
				for warning in &o.warnings {
					println!("warning: {warning}");
				}
			});
		}
		TagCmd::Delete(args) => {
			let input: sd_core::ops::tags::delete::input::DeleteTagInput = args.into();
			let out: DeleteTagOutput = execute_action!(ctx, input);
			print_output!(ctx, &out, |o: &DeleteTagOutput| {
				println!(
					"Removed {} application(s) across {} source(s)",
					o.applications_removed, o.sources_updated
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
				for tag in &o.tags {
					println!("{} {}", tag.id, tag.path);
				}
			});
		}
	}
	Ok(())
}
