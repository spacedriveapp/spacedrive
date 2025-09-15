use clap::Args;
use uuid::Uuid;

use sd_core::ops::libraries::{
    create::input::LibraryCreateInput,
    delete::input::LibraryDeleteInput,
    session::set_current::SetCurrentLibraryInput,
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
pub struct LibrarySwitchArgs {
    pub id: Uuid,
}

impl From<LibrarySwitchArgs> for SetCurrentLibraryInput {
    fn from(args: LibrarySwitchArgs) -> Self {
        Self {
            library_id: args.id,
        }
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

