use clap::Args;
use uuid::Uuid;

use sd_core::ops::libraries::{
	create::input::LibraryCreateInput, delete::input::LibraryDeleteInput,
	info::query::LibraryInfoQueryInput,
};

#[derive(Args, Debug)]
pub struct LibraryCreateArgs {
	pub name: String,
}

impl From<LibraryCreateArgs> for LibraryCreateInput {
	fn from(args: LibraryCreateArgs) -> Self {
		Self::new(args.name)
	}
}

#[derive(Args, Debug)]
pub struct LibraryDeleteArgs {
	pub library_id: Uuid,
	#[arg(long, short = 'y', default_value_t = false)]
	pub yes: bool,
	#[arg(long, default_value_t = false)]
	pub delete_data: bool,
}

impl From<LibraryDeleteArgs> for LibraryDeleteInput {
	fn from(args: LibraryDeleteArgs) -> Self {
		Self {
			library_id: args.library_id,
			delete_data: args.delete_data,
		}
	}
}

#[derive(Args, Debug)]
pub struct LibraryInfoArgs {
	/// Library ID to get information about (optional, defaults to current library)
	pub library_id: Option<Uuid>,
}

impl LibraryInfoArgs {
	/// Create an input for the specified library ID or current library
	pub fn to_input(&self, current_library_id: Option<Uuid>) -> anyhow::Result<LibraryInfoQueryInput> {
		let library_id = self
			.library_id
			.or(current_library_id)
			.ok_or_else(|| anyhow::anyhow!("No library specified and no current library set"))?;

		Ok(LibraryInfoQueryInput)
	}
}
