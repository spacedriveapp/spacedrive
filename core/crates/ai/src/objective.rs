use crate::{concept::Concept, define_concept, Prompt, SchemaProvider};
use schemars::{schema_for, JsonSchema};
use serde::{Deserialize, Serialize};
// The Objective concept is used to track progress for tasks large and small.
// Objectives can be put aside for more important tasks, but should be revisited regularly to ensure they are completed and/or archived.
#[derive(Prompt, JsonSchema, Debug, Clone, Default, Serialize, Deserialize)]
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
	pub due: Option<String>,
	#[prompt(
		instruct = "On a scale of 1-100, how much do we value this objective? Don't be afraid to set a high priority value for important tasks, especially if there are not many active ones.",
		default = 5
	)]
	pub priority: u16,
}

define_concept!(Objective);
