#![recursion_limit = "256"]
#![warn(clippy::unwrap_used, clippy::panic)]

pub(crate) mod object;
pub(crate) mod p2p;

#[doc(hidden)] // TODO(@Oscar): Make this private when breaking out `utils` into `sd-utils`
pub mod util;

// TODO: expose Node and API
