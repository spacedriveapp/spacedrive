pub mod action;
pub mod capability;
pub mod concept;
pub mod instruct;
pub mod journal;
pub mod model;
pub mod objective;
pub mod ollama;
pub mod working_memory;
use std::time::Duration;

pub use capability::{Capability, CAPABILITY_REGISTRY};
pub use concept::{Concept, CONCEPT_REGISTRY};
use futures::StreamExt;
use model::{ModelEvent, ModelResponse};
use ollama::OllamaClient;
use sd_prompt_derive::Prompt;

use tokio::{sync::mpsc, time::sleep};

pub trait Prompt {
	fn generate_prompt(&self) -> String;
}

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
}

impl ModelInstance {
	pub async fn start(&mut self) {
		let ollama_client = OllamaClient::new("http://localhost:11434"); // Adjust URL as needed

		loop {
			// Process any incoming events
			while let Ok(event) = self.event_rx.try_recv() {
				// Handle the event (e.g., update working memory)
				self.handle_event(event);
			}

			// Generate the prompt for the current stage
			let prompt = self.generate_prompt_for_stage();

			// Send the prompt to Ollama and get the response
			let (stream, _cancel_tx) = ollama_client
				.stream_prompt_with_cancellation(&prompt)
				.await
				.expect("Failed to start stream");

			// Pin the stream
			let mut pinned_stream = Box::pin(stream);

			let mut response_text = String::new();
			while let Some(chunk_result) = pinned_stream.next().await {
				match chunk_result {
					Ok(chunk) => response_text.push_str(&chunk),
					Err(_) => break,
				}
			}

			// Parse the JSON response
			let model_response: ModelResponse = match serde_json::from_str(&response_text) {
				Ok(response) => response,
				Err(e) => {
					eprintln!(
						"Failed to parse model response: {}. Response text: {}",
						e, response_text
					);
					continue; // Skip to the next iteration if parsing fails
				}
			};

			// Process the response
			self.process_model_response(model_response);

			// Short delay to prevent tight looping
			sleep(Duration::from_millis(100)).await;
		}
	}

	fn handle_event(&mut self, _event: ModelEvent) {
		// Update working memory based on the event
		self.working_memory.last_updated_at = chrono::Utc::now();
		// Add more event handling logic as needed
	}

	fn generate_prompt_for_stage(&self) -> String {
		// TODO: Implement prompt generation based on the current stage
		// This will be implemented in the next step
		String::new()
	}

	fn process_model_response(&mut self, response: ModelResponse) {
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
