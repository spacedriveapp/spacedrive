//! # Undo
//!
//! Reverse what a job did, from its journal, on preflight: validation says
//! what cannot be reversed and what changed since, the preview is the
//! reverse as an `FsPlan`, and the job applies it newest effect first,
//! writing its own journal so an undo can itself be undone.

pub mod action;
pub mod input;
pub mod job;
mod preflight;
pub mod reverse;
#[cfg(test)]
mod tests;

pub use action::FileUndoAction;
pub use input::FileUndoInput;
pub use job::{UndoJob, UndoOutput};
