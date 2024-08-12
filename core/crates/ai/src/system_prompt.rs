use crate::capability::CapabilityRequest;
use crate::concept::{AnyConceptWrapper, ConceptRequest, CONCEPT_REGISTRY};
use crate::instruct::BASE_INSTRUCT;
use crate::{objective::Objective, Capability, Prompt, CAPABILITY_REGISTRY};
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::{HashMap, VecDeque};

pub struct ProcessConfig {
	pub required_concepts: Vec<&'static str>,
	pub required_capabilities: Vec<&'static str>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SystemResponse {
	pub action: ResponseAction,
	pub details: String,
	pub requested_concepts: Vec<ConceptRequest>,
	pub requested_capabilities: Vec<CapabilityRequest>,
	pub output: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum ResponseAction {
	Continue,
	RequestMore,
	Complete,
}

pub struct SystemPrompt {
	process_queue: VecDeque<ProcessConfig>,
	objectives: VecDeque<Objective>,
	concepts: Vec<AnyConceptWrapper>,
	capabilities: Vec<Box<dyn Capability>>,
}

impl SystemPrompt {
	pub fn new() -> Self {
		Self {
			process_queue: VecDeque::new(),
			objectives: VecDeque::new(),
			concepts: Vec::new(),
			capabilities: Vec::new(),
		}
	}

	pub fn add_process(&mut self, config: ProcessConfig) {
		self.process_queue.push_back(config);
	}

	pub fn add_objective(&mut self, objective: Objective) {
		self.objectives.push_back(objective);
	}

	pub fn register_concept<T: crate::concept::Concept + 'static>(&mut self, concept: T) {
		self.concepts.push(AnyConceptWrapper::new(concept));
	}

	pub fn register_capability(&mut self, capability: Box<dyn Capability>) {
		self.capabilities.push(capability);
	}

	pub fn generate_prompt(&self) -> String {
		let mut prompt = String::new();

		// Base Instructions
		prompt.push_str("### Base Instructions:\n");
		prompt.push_str(BASE_INSTRUCT);
		prompt.push_str("\n\n");

		// Welcome Message and Concepts
		prompt.push_str("### Welcome to the AI System.\n");
		prompt.push_str("#### Available Concepts:\n");
		for concept in &self.concepts {
			prompt.push_str(&format!("- {}\n", concept.concept_name()));
		}

		// Capabilities
		prompt.push_str("\n#### Available Capabilities:\n");
		for capability in &self.capabilities {
			prompt.push_str(&format!(
				"- {}: {}\n",
				capability.name(),
				capability.description()
			));
		}

		// Process Queue
		if !self.process_queue.is_empty() {
			prompt.push_str("\n#### Current Process Queue:\n");
			for (i, config) in self.process_queue.iter().enumerate() {
				prompt.push_str(&format!(
					"{}. Required Concepts: {:?}, Required Capabilities: {:?}\n",
					i + 1,
					config.required_concepts,
					config.required_capabilities
				));
			}
			prompt.push_str("\nPlease address the processes in order. Use the concepts and capabilities as needed.\n");
		}

		// Registered Concepts Directory
		prompt.push_str("\n### Registered Concepts Directory:\n");
		let concepts = CONCEPT_REGISTRY.lock().unwrap();
		for (name, instances) in concepts.iter() {
			prompt.push_str(&format!("{}: {} instances\n", name, instances.len()));
		}

		// Response Format Instructions
		prompt.push_str("\n### Response Format:\n");
		prompt.push_str(&self.generate_response_format_instructions());

		prompt
	}

	// New method to handle user queries
	pub fn handle_query(&self, query: &str) -> String {
		let mut response = String::new();
		response.push_str("Processing your query...\n");

		// Here you would add logic to handle different types of queries
		// For now, just a placeholder to echo the query
		response.push_str(&format!("You asked: '{}'\n", query));
		response.push_str("Response: [Logic to process the query and provide output]\n");

		response
	}

	fn generate_response_format_instructions(&self) -> String {
		let format = serde_json::json!({
			"action": ["Continue", "RequestMore", "Complete"],
			"details": "Description of your action or request",
			"requested_concepts": [
				{
					"name": "ConceptName",
					"filters": {
						"key": "value"
					}
				}
			],
			"requested_capabilities": [
				{
					"name": "CapabilityName",
					"args": ["arg1", "arg2"]
				}
			],
			"output": "Your output or null if not applicable"
		});

		let formatted_json = serde_json::to_string_pretty(&format).unwrap();

		format!(
			"Please respond with a JSON object in the following format:\n\
            {}\n\
            Replace the example values with your actual response.\n\
            The 'action' field must be one of the provided options.\n",
			formatted_json
		)
	}
}
