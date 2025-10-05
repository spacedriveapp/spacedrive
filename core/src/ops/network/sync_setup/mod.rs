//! Library sync setup operations after pairing
//!
//! This module handles the setup of library synchronization between paired devices.
//! It is separate from the pairing protocol itself to maintain clean separation between
//! networking concerns (device pairing) and application concerns (library sync setup).

pub mod action;
pub mod discovery;
pub mod input;
pub mod output;

pub use action::LibrarySyncSetupAction;
pub use discovery::*;
pub use input::*;
pub use output::*;
