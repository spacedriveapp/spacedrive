use crate::Prompt;
use crate::{action::Action, concept::ConceptMeta};
use serde::{Deserialize, Serialize};
// Working memory is accessible throughout the execution process and gives the model a clear view of the current state of the system.
// Elements from here will be included in the system prompt
// This struct cannot be given a Prompt derive because it is exclusive to system memory
pub struct WorkingMemory {
	// any natural language notes that need to be saved during processing
	pub notes: Vec<Note>,
	// As concepts are chosen they are added to this list
	pub concepts: Vec<ConceptMeta>,
	// which stage of the process are we in
	pub stage: ProcessStage,

	pub started_at: chrono::DateTime<chrono::Utc>,
	pub last_updated_at: chrono::DateTime<chrono::Utc>,

	pub action_history: Vec<Box<Action>>,
}

impl WorkingMemory {
	pub fn new() -> Self {
		Self {
			notes: Vec::new(),
			concepts: Vec::new(),
			stage: ProcessStage::Idle,
			started_at: chrono::Utc::now(),
			last_updated_at: chrono::Utc::now(),
			action_history: Vec::new(),
		}
	}

	pub fn add_concept(&mut self, concept: ConceptMeta) {
		self.concepts.push(concept);
	}

	pub fn add_action(&mut self, action: Action) {
		self.action_history.push(Box::new(action));
	}
}

#[derive(Prompt, Debug, Clone, Serialize, Deserialize)]
pub struct Note {
	pub text: String,
	pub timestamp: chrono::DateTime<chrono::Utc>,
}

// We use the Prompt metadata when the system prompt is constructed
#[derive(Prompt, Debug, Clone, Serialize, Deserialize)]
#[prompt(
	instruct = "This is the state of the overall system.",
	show_variants = true
)]
pub enum ProcessStage {
	// An idle state is important to ensure execution loop doesn't needless run
	#[prompt(
		instruct = "The system is currently idle and waiting for input. This is the default state when no other actions are being taken. This will pause the execution loop."
	)]
	Idle,
	// This is the "main menu" state where the model can decide what needs to happen to move forward with the user's request
	#[prompt(
		instruct = "Evaluate the user input and determine which concepts are relevant. The concepts you chose will expand in the next prompt. If you do not need anything more you can just reply to the user directly and move to idle."
	)]
	Evaluate,
	#[prompt(
		instruct = "Now you have the concepts you need and the parameters required to manipulate them. From here you can choose to execute a capability and move forward to the next stage."
	)]
	Think,
	#[prompt(
		instruct = "Execute the selected capability or plan based on your current thought process. Ensure all actions are aligned with the objectives and the desired outcomes."
	)]
	Act,
	#[prompt(
		instruct = "Reflect on the steps taken and the outcomes achieved, create new [Memories] and [Objectives] if necessary. This is the final stage before moving back to idle."
	)]
	Reflect,
}
