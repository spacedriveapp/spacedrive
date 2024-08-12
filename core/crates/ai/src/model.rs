use crate::concept::Concept;
use crate::concept::ConceptRequest;
use crate::define_concept;
use crate::working_memory::ProcessStage;
use crate::Capability;
use crate::Prompt;
use chrono::prelude::*;
use serde::{Deserialize, Serialize};

// The Model will be forced to always respond with this structured response.
// Most of the model's functionality will happen through concepts.
#[derive(Serialize, Deserialize, Debug, Prompt)]
pub struct ModelResponse {
	#[prompt(
		instruct = "Select the snake_case identifiers of the concepts you want to expand in the next iteration."
	)]
	// if this is filled out the next system prompt will include this concept expanded
	pub request_concepts: Vec<String>,
	#[prompt(
		instruct = "Staying aware of any active [Conversation] with the user, if now is a good time to reply you may provide a message here and it will be added to the sent to the active conversation."
	)]
	// each round the model can optionally provide a message to the user
	pub message_for_user: Option<String>,
	// model can instruct system to advance to next stage
	pub next_stage: ProcessStage,
	// a brief overview of what happened in this round
	pub description: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ResponseAction {
	Execute,
	Respond,
	Store,
	Forget,
}

#[derive(Prompt, Debug, Clone)]
pub struct ModelEvent {
	pub r#type: ModelEventType,
	pub text: String,
	pub timestamp: DateTime<Utc>,
}
define_concept!(ModelEvent);

#[derive(Prompt, Debug, Clone)]
pub enum ModelEventType {
	Message,
	Action,
}
