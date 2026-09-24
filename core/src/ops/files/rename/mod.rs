//! # Rename
//!
//! One file to a new name, or many files by rules applied to each name in
//! order, both on preflight: validation resolves every new name against the
//! live directory and the target volume's filesystem, the preview is one
//! move per changed name, and the job renames in an order that never
//! overwrites and journals each move.

pub mod action;
pub mod input;
pub mod job;
pub mod naming;
mod preflight;
pub mod resolve;
pub mod rules;

pub use action::{FileRenameAction, FileRenameBatchAction};
pub use input::{FileRenameBatchInput, FileRenameInput};
pub use job::{RenameJob, RenameOutput};
pub use naming::{check_name, check_portable, NameProblem, NameRules};
pub use rules::{CaseRule, ExtensionCase, RenameRule};
