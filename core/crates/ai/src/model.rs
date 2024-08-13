use crate::concept::Concept;
use crate::define_concept;
use crate::working_memory::ProcessStage;
use crate::Prompt;
use chrono::prelude::*;
use serde::{Deserialize, Serialize};

// The Model will be forced to always respond with this structured response.
// Later on we can make the response options dynamic and add new ones.
#[derive(Serialize, Deserialize, Debug, Prompt)]
pub struct ModelResponse {
	#[prompt(
		instruct = "Select the snake_case identifiers of any Concepts you want to expand in the next iteration, to provide you with the exact instructions and parameter specifications you need."
	)]
	// if this is filled out the next system prompt will include this concept expanded
	pub request_concepts: Vec<String>,
	#[prompt(
		instruct = "Staying aware of any active [Conversation] with the user, if now is a good time to reply you may provide a message here and it will be added to the sent to the active conversation."
	)]
	pub message_for_user: Option<String>,
	// model can instruct system to advance to next stage
	pub next_stage: ProcessStage,
	// a brief overview of what happened in this round
	pub description: Option<String>,
}

// The ModelEvent is created in code by the system, but is viewable by the model so we define as a Concept.
#[derive(Prompt, Debug, Clone, Default)]
#[prompt(
	instruct = "Events are created by the system to inform the model of important changes or actions taken."
)]
pub struct ModelEvent {
	pub r#type: ModelEventType,
	pub text: String,
	pub timestamp: DateTime<Utc>,
}
define_concept!(ModelEvent);

#[derive(Prompt, Debug, Clone, Default)]
pub enum ModelEventType {
	#[default]
	SystemMessage,
	UserMessage,
	PerformedAction,
}
