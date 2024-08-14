use action::Action;
pub use capability::{Capability, CAPABILITY_REGISTRY};
use chrono::{DateTime, Utc};
use colored::*;
use concept::list_concepts;
pub use concept::{Concept, SchemaProvider};
use futures::{Stream, StreamExt};
use journal::{Journal, JournalEntry};
use model::{ModelEvent, ModelEventType, ModelResponse};
use objective::Objective;
use ollama::OllamaClientWrapper;
pub use prompt_factory::*;
use sd_core_prompt_derive::Prompt;
use serde_json;
use std::{pin::Pin, time::Duration};
use tokio::{sync::mpsc, time::sleep};
use working_memory::{ProcessStage, UserMessage};

pub mod action;
pub mod capability;
pub mod concept;
pub mod instruct;
pub mod journal;
pub mod model;
pub mod objective;
pub mod ollama;
pub mod prompt_factory;
pub mod working_memory;

pub struct ModelInstance {
	pub working_memory: working_memory::WorkingMemory,
	pub context_window_length: usize,
	pub model_name: String,
	pub event_tx: mpsc::Sender<ModelEvent>,
	pub event_rx: mpsc::Receiver<ModelEvent>,
	concept_initializers: Vec<Box<dyn Fn() + Send + Sync>>,
	pub ollama_client: OllamaClientWrapper, // Changed to OllamaClientWrapper
}

impl ModelInstance {
	pub fn new() -> Self {
		let (event_tx, event_rx) = mpsc::channel(100);

		let mut instance = Self {
			working_memory: working_memory::WorkingMemory::new(),
			context_window_length: 5,
			model_name: "llama2:70b".to_string(),
			event_tx,
			event_rx,
			concept_initializers: Vec::new(),
			ollama_client: OllamaClientWrapper::new("http://localhost", 11434), // Modified to match the type
		};

		// Populate the vector with concept registration functions
		// instance.add_root_concept::<Action>();
		instance.add_root_concept::<Objective>();
		instance.add_root_concept::<Journal>();
		// instance.add_root_concept::<ModelEvent>();

		// Register all concepts
		instance.register_concepts();

		instance
	}

	fn add_root_concept<T: Concept + Default + 'static>(&mut self) {
		self.concept_initializers.push(Box::new(|| {
			T::concept_name(); // This ensures the concept gets registered
		}));
	}

	fn register_concepts(&self) {
		for init in &self.concept_initializers {
			init();
		}
	}

	pub async fn start(&mut self) {
		loop {
			while let Ok(event) = self.event_rx.try_recv() {
				self.handle_event(event);
			}

			if let ProcessStage::Idle = self.working_memory.stage {
				sleep(Duration::from_millis(10000)).await;
				continue;
			}

			let prompt = self.generate_system_prompt();

			let (stream, _cancel_tx) = self
				.ollama_client
				.generate_response_stream(&prompt) // Updated to use generate_response_stream
				.await
				.expect("Failed to start stream");

			let mut pinned_stream: Pin<
				Box<
					dyn Stream<Item = Result<String, Box<dyn std::error::Error + Send + Sync>>>
						+ Send
						+ 'static,
				>,
			> = Box::pin(stream);

			let mut response_text = String::new();
			while let Some(chunk_result) = pinned_stream.next().await {
				match chunk_result {
					Ok(content) => {
						response_text.push_str(&content);
					}
					Err(e) => {
						eprintln!("Error receiving chunk: {}", e);
						break;
					}
				}
			}

			match serde_json::from_str::<ModelResponse>(&response_text) {
				Ok(model_response) => {
					self.process_model_response(model_response);
				}
				Err(e) => {
					// set to idle state
					self.working_memory.stage = ProcessStage::Idle;
					eprintln!(
						"Failed to parse model response: {}. Response text: {}",
						e, response_text
					);
				}
			}

			sleep(Duration::from_millis(1000)).await;
		}
	}

	fn handle_event(&mut self, event: ModelEvent) {
		match event.r#type {
			ModelEventType::UserMessage(message) => {
				println!("Received user message: {}", message);
				// Store user message with timestamp and unread status
				self.working_memory.user_inputs.push(UserMessage {
					message,
					timestamp: Utc::now().to_string(),
					read: false,
				});

				// Trigger the loop if idle
				if self.working_memory.stage == ProcessStage::Idle {
					self.working_memory.stage = ProcessStage::Evaluate;
				}
			}
			ModelEventType::PerformedAction(action) => {
				println!("Performing system action: {:?}", action);
				// Implement system action handling
				// For example, update working memory, trigger capabilities, etc.
			} // Add more event types as needed
			_ => {
				println!("Received event: {:?}", event);
				// Handle other event types
			}
		}
		// Update working memory based on the event
		self.working_memory.last_updated_at = Utc::now();
	}

	pub fn generate_system_prompt(&self) -> String {
		let mut prompt = PromptFactory::new();

		prompt.add_section(
			format!("Current Stage: {:?}", &self.working_memory.stage),
			&self.working_memory.stage,
		);

		if !self.working_memory.notes.is_empty() {
			prompt.add_section_grouped("Notes".to_string(), self.working_memory.notes.clone());
		}

		prompt.add_section_grouped("Concepts".to_string(), list_concepts());

		// Handle user input
		if let Some(user_input) = self
			.working_memory
			.user_inputs
			.iter()
			.find(|input| !input.read)
		{
			prompt.add_text_section("User Input".to_string(), user_input.message.clone());
		}

		let prompt = prompt.finalize();

		// Print the prompt in a formatted way
		println!("{}", "Generated Prompt:".green().bold());
		println!("{}", "=".repeat(50).yellow());

		for line in prompt.lines() {
			if line.starts_with("###") {
				println!("\n{}", line.cyan().bold());
			} else {
				println!("{}", line);
			}
		}
		println!("{}", "=".repeat(50).yellow());

		prompt
	}
	fn process_model_response(&mut self, response: ModelResponse) {
		println!("{}", "Processing model response:".blue().bold());
		println!(
			"{}",
			serde_json::to_string_pretty(&response).unwrap().cyan()
		);

		// // Check for the presence of next_stage
		// if let Some(next_stage) = response.next_stage {
		// 	self.working_memory.stage = next_stage;

		// 	// If the next stage is `Idle`, transition and clear actions
		// 	if let ProcessStage::Idle = next_stage {
		// 		self.working_memory.user_inputs.clear();
		// 		self.working_memory.action_history.clear();
		// 	}
		// } else {
		// 	// Handle the error - Missing `next_stage`
		// 	eprintln!("Error: `next_stage` is missing in the model response.");
		// 	// Stop the loop or transition to an error-handling state
		// 	self.working_memory.stage = ProcessStage::Idle; // or another appropriate state
		// }

		// Handle requested concepts
		for concept_request in response.request_concepts {
			println!("Requested concept: {:?}", concept_request);
			// Update working memory with the requested concept if needed
		}

		// Handle user message if present
		if let Some(message) = response.message_for_user {
			println!("{} {}", "Message for user:".green().bold(), message);
			// Mark all unread user inputs as read if a response is sent
			for user_input in &mut self.working_memory.user_inputs {
				user_input.read = true;
			}
		}

		// Update the last_updated_at timestamp
		self.working_memory.last_updated_at = Utc::now();
	}
}
