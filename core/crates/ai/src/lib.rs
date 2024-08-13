use action::Action;
pub use capability::{Capability, CAPABILITY_REGISTRY};
use concept::list_concepts;
pub use concept::{Concept, SchemaProvider};
use journal::{Journal, JournalEntry};
use model::{ModelEvent, ModelResponse};
// use ollama::OllamaClient;
use colored::*;
use objective::Objective;
pub use prompt_factory::*;
use sd_core_prompt_derive::Prompt;
use std::time::Duration;
use tokio::{sync::mpsc, time::sleep};

pub mod action;
pub mod capability;
pub mod concept;
pub mod instruct;
pub mod journal;
pub mod model;
pub mod objective;
// pub mod ollama;
pub mod prompt_factory;
pub mod working_memory;

pub struct ModelInstance {
	// TODO: to add concurrency multiple instances of working memory might allow for parallel processing and fast context switching, like tabs.
	pub working_memory: working_memory::WorkingMemory,
	// this will allow us to ensure our prompt generation doesn't exceed the context window
	pub context_window_length: usize,
	// embedded in the base system prompt
	pub model_name: String,
	// channels for communication with the outside
	pub event_tx: mpsc::Sender<ModelEvent>,
	pub event_rx: mpsc::Receiver<ModelEvent>,
	concept_initializers: Vec<Box<dyn Fn() + Send + Sync>>,
}

impl ModelInstance {
	pub fn new() -> Self {
		let (event_tx, event_rx) = mpsc::channel(100);

		let mut instance = Self {
			working_memory: working_memory::WorkingMemory::new(),
			context_window_length: 5,
			model_name: "llama3.1:70b".to_string(),
			event_tx,
			event_rx,
			concept_initializers: Vec::new(),
		};

		// Populate the vector with concept registration functions
		// Root concepts are shown on the main menu
		instance.add_root_concept::<Action>();
		instance.add_root_concept::<Objective>();
		instance.add_root_concept::<Journal>();
		instance.add_root_concept::<JournalEntry>();
		instance.add_root_concept::<ModelEvent>();

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
		// let ollama_client = OllamaClient::new("http://localhost:11434"); // Adjust URL as needed

		loop {
			// // Process any incoming events
			// while let Ok(event) = self.event_rx.try_recv() {
			// 	match event {
			// 		ModelEvent::Message => {
			// 			// Handle message event
			// 		}
			// 		ModelEvent::Action => !unimplemented!(),
			// 	}
			// }

			// // Generate the prompt for the current stage
			// let prompt = self.generate_system_prompt();

			// // Send the prompt to Ollama and get the response
			// let (stream, _cancel_tx) = ollama_client
			// 	.stream_prompt_with_cancellation(&prompt)
			// 	.await
			// 	.expect("Failed to start stream");

			// // Pin the stream
			// let mut pinned_stream = Box::pin(stream);

			// let mut response_text = String::new();
			// while let Some(chunk_result) = pinned_stream.next().await {
			// 	match chunk_result {
			// 		Ok(chunk) => response_text.push_str(&chunk),
			// 		Err(_) => break,
			// 	}
			// }

			// // Parse the JSON response
			// let model_response: ModelResponse = match serde_json::from_str(&response_text) {
			// 	Ok(response) => response,
			// 	Err(e) => {
			// 		eprintln!(
			// 			"Failed to parse model response: {}. Response text: {}",
			// 			e, response_text
			// 		);
			// 		continue; // Skip to the next iteration if parsing fails
			// 	}
			// };

			// // Process the response
			// self.process_model_response(model_response);

			// Short delay to prevent tight looping
			sleep(Duration::from_millis(1000)).await;
		}
	}

	fn handle_event(&mut self, _event: ModelEvent) {
		// Update working memory based on the event
		self.working_memory.last_updated_at = chrono::Utc::now();
		// Add more event handling logic as needed
	}

	pub fn generate_system_prompt(&self) -> String {
		let mut prompt = PromptFactory::new();

		prompt.add_section("Current Stage".to_string(), &self.working_memory.stage);

		if !self.working_memory.notes.is_empty() {
			prompt.add_section_grouped("Notes".to_string(), self.working_memory.notes.clone());
		}

		prompt.add_section_grouped("Concepts".to_string(), list_concepts());

		prompt.add_text_section(
			"User Input".to_string(),
			"I want to start a meal plan".to_string(),
		);

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
		println!();

		prompt
	}

	fn process_model_response(&mut self, response: ModelResponse) {
		println!("Processing model response: {:?}", response);
		// Update the working memory with the new stage
		self.working_memory.stage = response.next_stage;

		// Handle requested concepts
		for _concept_request in response.request_concepts {
			// TODO: Implement concept retrieval and addition to working memory
		}

		// Handle user message if present
		if let Some(_message) = response.message_for_user {
			// TODO: Implement user message handling (e.g., send to a UI)
		}

		// Update the last_updated_at timestamp
		self.working_memory.last_updated_at = chrono::Utc::now();
	}
}
