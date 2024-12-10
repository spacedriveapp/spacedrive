/// LLM Engine
///
/// This module contains the LLM engine for the Spacedrive core.
/// It is responsible for generating and executing AI tasks.
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

pub mod agents;
pub mod config;
pub mod memory;
pub mod tools;

// Core error type for LLM operations
#[derive(Error, Debug)]
pub enum LLMError {
	#[error("Configuration error: {0}")]
	Config(String),
	#[error("Model error: {0}")]
	Model(String),
	#[error("Tool execution error: {0}")]
	Tool(String),
	#[error("Memory error: {0}")]
	Memory(String),
}

// Result type alias for LLM operations
pub type Result<T> = std::result::Result<T, LLMError>;

// Core agent configuration structures
#[derive(Debug, Deserialize, Serialize)]
pub struct AgentConfig {
	pub name: String,
	pub description: String,
	pub model: ModelConfig,
	pub tools: Vec<ToolConfig>,
	pub workflow: WorkflowConfig,
	pub memory: MemoryConfig,
	pub prompts: PromptTemplates,
	pub validation: ValidationConfig,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ModelConfig {
	pub provider: String,
	pub name: String,
	pub temperature: f32,
	pub max_tokens: usize,
	pub system_prompt: String,
}

// Core agent trait
#[async_trait]
pub trait Agent: Send + Sync {
	async fn execute(&self, query: &str) -> Result<String>;
	async fn load_tools(&mut self) -> Result<()>;
	async fn initialize_memory(&mut self) -> Result<()>;
}

// Core tool trait
#[async_trait]
pub trait Tool: Send + Sync {
	async fn execute(
		&self,
		params: HashMap<String, serde_json::Value>,
	) -> Result<serde_json::Value>;
	fn name(&self) -> &str;
	fn description(&self) -> &str;
}

// Re-exports
pub use self::agents::DirectoryAgent;
pub use self::config::ConfigParser;
pub use self::memory::MemoryManager;
pub use self::tools::SpacedriveFs;
