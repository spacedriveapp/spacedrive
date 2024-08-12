pub mod capability;
pub mod concept;
pub mod instruct;
pub mod journal;
pub mod objective;
pub mod system_prompt;

pub use capability::{Capability, CAPABILITY_REGISTRY};
pub use concept::{Concept, CONCEPT_REGISTRY};
pub use system_prompt::{ProcessConfig, ResponseAction, SystemPrompt, SystemResponse};

use chrono::prelude::*;
use sd_prompt_derive::Prompt;
use tokio::sync::mpsc;

pub trait Prompt {
	fn generate_prompt(&self) -> String;
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

#[derive(Prompt, Debug, Clone)]
pub enum ModelInputType {
	Command, // one-shot
	Query,   // one-shot
	Conversation,
}

pub struct ModelInstance {
	pub id: i64,
	pub state: SystemState,
	pub system_prompt: SystemPrompt,
	pub event_tx: mpsc::Sender<ModelEvent>,
	pub event_rx: mpsc::Receiver<ModelEvent>,
}

impl ModelInstance {
	pub fn new(id: i64) -> Self {
		let (event_tx, event_rx) = mpsc::channel(100);
		let system_prompt = SystemPrompt::new();

		ModelInstance {
			id,
			state: SystemState::Idle,
			system_prompt,
			event_tx,
			event_rx,
		}
	}

	pub async fn start(&mut self) {
		loop {
			match self.state {
				SystemState::Idle => {
					if let Some(event) = self.event_rx.recv().await {
						self.handle_event(event).await;
					}
				}
				SystemState::Routing => {
					// Determine the next action based on current objectives, memories, and context
					// Simulate routing logic
					self.state = SystemState::Acting;
				}
				SystemState::Acting => {
					// Execute actions and capabilities
					self.execute_current_task().await;
					self.state = SystemState::Reflecting;
				}
				SystemState::Reflecting => {
					// Reflect on the outcomes and update strategies
					self.state = SystemState::Idle;
				}
			}
		}
	}

	async fn handle_event(&mut self, event: ModelEvent) {
		match event.r#type {
			ModelEventType::Message => {
				println!("Received message: {}", event.text);
				// Process message and decide on the next action
				self.state = SystemState::Routing;
			}
			ModelEventType::Action => {
				println!("Executing action: {}", event.text);
				// Execute specific action logic
				self.state = SystemState::Acting;
			}
		}
	}

	async fn execute_current_task(&self) {
		// Logic to execute the current task, utilizing capabilities or interacting with concepts
		println!("Executing task in Acting state.");
	}

	pub async fn send_message(&self, message: String) {
		let event = ModelEvent {
			r#type: ModelEventType::Message,
			text: message,
			timestamp: Utc::now(),
		};
		self.event_tx.send(event).await.unwrap();
	}
}

#[derive(Prompt, Debug, Clone)]
#[prompt(
	instruct = "Define your current state and execute the appropriate behavior for each state."
)]
pub enum Stage {
	Plan, // planning and tool selection
	Execute,
	Reflect,
}

// Actions are created when
#[derive(Prompt, Debug, Clone)]
#[prompt(meaning = r###"
        The system will create actions for all capabilities executed. You can review these at any time to ensure the system is behaving as expected. Ensure we are not looping or stuck in a state.
    "###)]
pub struct Action {
	pub name: String,
	pub description: String,
	pub stage: Stage,
}
define_concept!(Action);

#[derive(Prompt)]
#[prompt(
	cardinality = "single",
	meaning = "This is the state of the overall system."
)]
pub enum SystemState {
	#[prompt(
		instruct = "Remain idle. Await new instructions or triggers before taking any action."
	)]
	Idle,
	#[prompt(
		instruct = "Determine the next best action or capability to invoke based on current objectives, memories, and available context. Make sure to prioritize effectively."
	)]
	Routing,
	#[prompt(
		instruct = "Execute the selected capability or plan based on your current thought process. Ensure all actions are aligned with the objectives and the desired outcomes."
	)]
	Acting,
	#[prompt(
		instruct = "Reflect on the actions you've taken. Analyze the outcomes, gather new data, and reassess your strategies and objectives. Make adjustments as needed based on what you've learned."
	)]
	Reflecting,
}

// Example capability and concept implementations would go here
