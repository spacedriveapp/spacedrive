//! Index Integrity Verification
//!
//! Verifies the integrity of the Spacedrive index by comparing the database state
//! with the actual filesystem state for a given path.

pub mod action;
pub mod input;
pub mod output;

pub use action::IndexVerifyAction;
pub use input::IndexVerifyInput;
pub use output::{IndexVerifyOutput, IntegrityDifference, IntegrityReport};
