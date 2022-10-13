#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![warn(clippy::correctness)]
#![warn(clippy::perf)]
#![warn(clippy::style)]
#![warn(clippy::suspicious)]
#![warn(clippy::nursery)]
#![warn(clippy::correctness)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::similar_names)]

pub mod error;
pub mod header;
pub mod keys;
pub mod objects;
pub mod primitives;
pub mod protected;

// Re-export this so that payloads can be generated elsewhere
pub use aead::Payload;
pub use zeroize::Zeroize;
