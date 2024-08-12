use crate::{capability::CapabilityRequest, concept::*, define_concept, Prompt};
use chrono::prelude::*;
use std::any::Any;

// Data driven human language system design.
#[derive(Prompt, Debug, Clone)]
#[prompt(
	instruct = "Use to track progress for tasks large and small. Objectives can be put aside for more important tasks, but should be revisited regularly to ensure they are completed and/or archived."
)]
pub struct Objective {
	#[prompt(
		instruct = "Describe the objective in 1-2 sentences. Be as specific as possible to ensure clarity and focus."
	)]
	pub description: String,
	pub complete: bool,
	pub active: bool,
	pub due: Option<DateTime<Utc>>,
	#[prompt(
		instruct = "On a scale of 1-100, how much do we value this objective? Don't be afraid to set a high priority value for important tasks, especially if there are not many active ones.",
		default = 5
	)]
	pub priority: u16,

	pub relevant_concepts: Vec<String>,
	pub relevant_capabilities: Vec<String>,
	// steps_taken: Vec<Step>,
	// recalled_memories: Vec<Memory>,
	// final_conclusions: Vec<Conclusion>,
}
define_concept!(Objective);
