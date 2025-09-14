use clap::Args;
use uuid::Uuid;

use sd_core::ops::libraries::{
    create::input::LibraryCreateInput,
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

