//! Builder pattern traits for actions

use std::error::Error;

/// Core trait for action builders
pub trait ActionBuilder {
    type Action;
    type Error: Error + Send + Sync + 'static;
    
    /// Validate the current builder state
    fn validate(&self) -> Result<(), Self::Error>;
    
    /// Build the final action instance
    fn build(self) -> Result<Self::Action, Self::Error>;
}

/// Trait for builders that can be constructed from CLI arguments
pub trait CliActionBuilder: ActionBuilder {
    type Args: clap::Parser;
    
    /// Create a builder from parsed CLI arguments
    fn from_cli_args(args: Self::Args) -> Self;
}

/// Errors that can occur during action building
#[derive(Debug, thiserror::Error)]
pub enum ActionBuildError {
    #[error("Validation errors: {0:?}")]
    Validation(Vec<String>),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Parse error: {0}")]
    Parse(String),
    
    #[error("Permission denied: {0}")]
    Permission(String),
    
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),
    
    #[error("Required field missing: {0}")]
    RequiredField(String),
}

impl ActionBuildError {
    /// Create a validation error with a single message
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation(vec![message.into()])
    }
    
    /// Create a validation error with multiple messages
    pub fn validations(messages: Vec<String>) -> Self {
        Self::Validation(messages)
    }
    
    /// Create a parse error
    pub fn parse(message: impl Into<String>) -> Self {
        Self::Parse(message.into())
    }
    
    /// Create a permission error
    pub fn permission(message: impl Into<String>) -> Self {
        Self::Permission(message.into())
    }
    
    /// Create an invalid argument error
    pub fn invalid_argument(message: impl Into<String>) -> Self {
        Self::InvalidArgument(message.into())
    }
    
    /// Create a required field error
    pub fn required_field(field: impl Into<String>) -> Self {
        Self::RequiredField(field.into())
    }
}