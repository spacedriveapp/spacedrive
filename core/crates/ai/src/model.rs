use crate::concept::*;
use crate::define_concept;
use crate::working_memory::ProcessStage;
use crate::Prompt;
use crate::SchemaProvider;
use chrono::prelude::*;
use schemars::{schema_for, JsonSchema};
use serde::{Deserialize, Serialize};

// The Model will be forced to always respond with this structured response.
// Later on we can make the response options dynamic and add new ones.
#[derive(Prompt, JsonSchema, Debug, Clone, Default, Serialize, Deserialize)]
#[prompt(
	instruct = "The ModelResponse is a structured response that the model must always respond with. It is used to instruct the system on what to do next and to provide a brief overview of what happened in this round."
)]
pub struct ModelResponse {
	// model can instruct system to advance to next stage
	pub next_stage: ProcessStage,
	// a brief overview of what happened in this round
	pub description: Option<String>,
	#[prompt(
		instruct = "The string value is JSON data, you MUST provide the parameters in the exact schema for this Concept."
	)]
	pub create_concepts: Option<Vec<String>>,
	#[prompt(
		instruct = "Select the identifiers of any Concepts you want to expand in the next iteration, to provide you with the exact instructions and parameter specifications you need. Only use this if you do not have the exact schema for the Concept present."
	)]
	// if this is filled out the next system prompt will include this concept expanded
	pub request_concepts: Option<Vec<String>>,
	#[prompt(
		instruct = "Staying aware of any active [Conversation] with the user, if now is a good time to reply you may provide a message here and it will be added to the sent to the active conversation."
	)]
	pub message_for_user: Option<String>,
}
define_concept!(ModelResponse);

// The ModelEvent is created in code by the system, but is viewable by the model so we define as a Concept.
#[derive(Prompt, JsonSchema, Debug, Clone, Default, Serialize, Deserialize)]
#[prompt(
	instruct = "Events are created by the system to inform the model of important changes or actions taken."
)]
pub struct ModelEvent {
	pub r#type: ModelEventType,
	pub text: String,
	pub timestamp: String,
}
define_concept!(ModelEvent);

#[derive(Prompt, JsonSchema, Debug, Clone, Default, Serialize, Deserialize)]
#[prompt(instruct = "The type of event that occurred.")]
pub enum ModelEventType {
	#[default]
	SystemMessage,
	UserMessage,
	PerformedAction,
}
define_concept!(ModelEventType);
