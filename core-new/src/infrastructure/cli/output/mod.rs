//! CLI Output System
//!
//! This module provides a structured and consistent output system for the CLI,
//! replacing direct `println!` calls with a type-safe, testable approach.
//!
//! # Features
//! - Multiple output formats (Human, JSON, Quiet)
//! - Color and emoji support with automatic fallback
//! - Testable output with writer injection
//! - Consistent styling across all commands
//! - Structured message types for all CLI outputs

pub mod context;
pub mod formatters;
pub mod messages;
pub mod section;

#[cfg(test)]
mod tests;

pub use context::{ColorMode, OutputContext, OutputFormat, VerbosityLevel};
pub use messages::Message;
pub use section::OutputSection;

use console::Emoji;
use std::io::{self, Write};

// Emoji constants with fallbacks
pub const SUCCESS: Emoji<'_, '_> = Emoji("✅ ", "[OK] ");
pub const ERROR: Emoji<'_, '_> = Emoji("❌ ", "[ERROR] ");
pub const WARNING: Emoji<'_, '_> = Emoji("⚠️  ", "[WARN] ");
pub const INFO: Emoji<'_, '_> = Emoji("ℹ️  ", "[INFO] ");
pub const SEARCH: Emoji<'_, '_> = Emoji("🔍 ", "[SEARCH] ");
pub const FOLDER: Emoji<'_, '_> = Emoji("📁 ", "[FOLDER] ");
pub const LIBRARY: Emoji<'_, '_> = Emoji("📚 ", "[LIBRARY] ");
pub const NETWORK: Emoji<'_, '_> = Emoji("🌐 ", "[NETWORK] ");
pub const DEVICE: Emoji<'_, '_> = Emoji("💻 ", "[DEVICE] ");
pub const ROCKET: Emoji<'_, '_> = Emoji("🚀 ", "[START] ");
pub const STOP: Emoji<'_, '_> = Emoji("🛑 ", "[STOP] ");
pub const MAIL: Emoji<'_, '_> = Emoji("📬 ", "[MAIL] ");
pub const TRASH: Emoji<'_, '_> = Emoji("🗑️  ", "[DELETE] ");
pub const CHART: Emoji<'_, '_> = Emoji("📊 ", "[STATUS] ");
pub const CLOCK: Emoji<'_, '_> = Emoji("🕐 ", "[TIME] ");
pub const PAIRING: Emoji<'_, '_> = Emoji("🔗 ", "[PAIR] ");
pub const BULB: Emoji<'_, '_> = Emoji("💡 ", "[TIP] ");
pub const CHECKMARK: Emoji<'_, '_> = Emoji("✓ ", "[OK] ");

/// Main CLI output handler
pub struct CliOutput {
    context: OutputContext,
}

impl CliOutput {
    /// Create a new output handler with the given format
    pub fn new(format: OutputFormat) -> Self {
        Self {
            context: OutputContext::new(format),
        }
    }

    /// Create a new output handler with all options
    pub fn with_options(
        format: OutputFormat,
        verbosity: VerbosityLevel,
        color: ColorMode,
    ) -> Self {
        Self {
            context: OutputContext::with_options(format, verbosity, color),
        }
    }

    /// Create a test output handler that captures output to a string
    #[cfg(test)]
    pub fn test() -> (Self, std::sync::Arc<std::sync::Mutex<Vec<u8>>>) {
        let buffer = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let writer = TestWriter { buffer: buffer.clone() };
        let context = OutputContext::test(Box::new(writer));
        (Self { context }, buffer)
    }

    /// Print a message
    pub fn print(&mut self, message: Message) -> io::Result<()> {
        self.context.print(message)
    }

    /// Print an error message (always prints regardless of verbosity)
    pub fn error(&mut self, message: Message) -> io::Result<()> {
        self.context.error(message)
    }

    /// Print a success message
    pub fn success(&mut self, text: &str) -> io::Result<()> {
        self.print(Message::Success(text.to_string()))
    }

    /// Print an info message
    pub fn info(&mut self, text: &str) -> io::Result<()> {
        self.print(Message::Info(text.to_string()))
    }

    /// Print a warning message
    pub fn warning(&mut self, text: &str) -> io::Result<()> {
        self.print(Message::Warning(text.to_string()))
    }

    /// Create a new output section builder
    pub fn section(&mut self) -> OutputSection {
        OutputSection::new(&mut self.context)
    }

    /// Get the test output buffer
    #[cfg(test)]
    pub fn test_buffer(&self) -> Option<String> {
        None // This will be handled differently
    }

    /// Check if output should use colors
    pub fn use_color(&self) -> bool {
        self.context.use_color()
    }

    /// Check if output should use emojis
    pub fn use_emoji(&self) -> bool {
        self.context.use_emoji()
    }

    /// Get the current output format
    pub fn format(&self) -> &OutputFormat {
        self.context.format()
    }

    /// Get the current verbosity level
    pub fn verbosity(&self) -> &VerbosityLevel {
        self.context.verbosity()
    }
}


// Test writer for capturing output
#[cfg(test)]
struct TestWriter {
    buffer: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
}

#[cfg(test)]
impl Write for TestWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buffer.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}