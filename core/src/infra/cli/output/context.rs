//! Output context and configuration

use super::formatters::{HumanFormatter, JsonFormatter, OutputFormatter};
use super::messages::Message;
use std::io::{self, Write};
use supports_color::Stream;

/// Output format for CLI commands
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// Human-readable output with colors and emojis
    Human,
    /// Machine-readable JSON output
    Json,
    /// Minimal output (errors only)
    Quiet,
}

impl OutputFormat {
    /// Parse from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "human" => Some(OutputFormat::Human),
            "json" => Some(OutputFormat::Json),
            "quiet" => Some(OutputFormat::Quiet),
            _ => None,
        }
    }
}

/// Verbosity level for output
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum VerbosityLevel {
    /// Errors only
    Quiet = 0,
    /// Default output
    Normal = 1,
    /// Additional information
    Verbose = 2,
    /// Everything including debug info
    Debug = 3,
}

impl VerbosityLevel {
    /// Create from occurrence count (e.g., -v, -vv, -vvv)
    pub fn from_occurrences(count: u8) -> Self {
        match count {
            0 => VerbosityLevel::Normal,
            1 => VerbosityLevel::Verbose,
            2.. => VerbosityLevel::Debug,
        }
    }
}

/// Color output mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    /// Automatically detect terminal support
    Auto,
    /// Always use colors
    Always,
    /// Never use colors
    Never,
}

/// Global output context passed through CLI operations
pub struct OutputContext {
    pub(super) format: OutputFormat,
    pub(super) verbosity: VerbosityLevel,
    pub(super) color: ColorMode,
    pub(super) writer: Box<dyn Write>,
    pub(super) formatter: Box<dyn OutputFormatter>,
    pub(super) use_emoji: bool,
    pub(super) use_color: bool,
}

impl OutputContext {
    /// Create a new output context with default settings
    pub fn new(format: OutputFormat) -> Self {
        Self::with_options(format, VerbosityLevel::Normal, ColorMode::Auto)
    }

    /// Create a new output context with all options
    pub fn with_options(
        format: OutputFormat,
        verbosity: VerbosityLevel,
        color: ColorMode,
    ) -> Self {
        let writer: Box<dyn Write> = Box::new(io::stdout());
        
        // Determine color support
        let use_color = match color {
            ColorMode::Always => true,
            ColorMode::Never => false,
            ColorMode::Auto => supports_color::on(Stream::Stdout).is_some(),
        };

        // Determine emoji support (use emojis if we have color support)
        let use_emoji = use_color && format != OutputFormat::Json;

        // Create appropriate formatter
        let formatter: Box<dyn OutputFormatter> = match format {
            OutputFormat::Human => Box::new(HumanFormatter::new(use_color, use_emoji)),
            OutputFormat::Json => Box::new(JsonFormatter),
            OutputFormat::Quiet => Box::new(HumanFormatter::new(false, false)),
        };

        Self {
            format,
            verbosity,
            color,
            writer,
            formatter,
            use_emoji,
            use_color,
        }
    }

    /// Create a test context with a custom writer
    #[cfg(test)]
    pub fn test(writer: Box<dyn Write>) -> Self {
        let formatter = Box::new(HumanFormatter::new(false, false));

        Self {
            format: OutputFormat::Human,
            verbosity: VerbosityLevel::Normal,
            color: ColorMode::Never,
            writer,
            formatter,
            use_emoji: false,
            use_color: false,
        }
    }

    /// Check if a message should be printed based on verbosity
    pub fn should_print(&self, message: &Message) -> bool {
        match self.format {
            OutputFormat::Quiet => message.is_error(),
            _ => {
                let msg_level = message.verbosity_level();
                msg_level <= self.verbosity
            }
        }
    }

    /// Print a message
    pub fn print(&mut self, message: Message) -> io::Result<()> {
        if self.should_print(&message) {
            let formatted = self.formatter.format(&message, self);
            writeln!(self.writer, "{}", formatted)?;
        }
        Ok(())
    }

    /// Print an error message (always prints)
    pub fn error(&mut self, message: Message) -> io::Result<()> {
        let formatted = self.formatter.format_error(&message, self);
        writeln!(self.writer, "{}", formatted)?;
        Ok(())
    }

    /// Write raw string (for section output)
    pub fn write(&mut self, content: &str) -> io::Result<()> {
        write!(self.writer, "{}", content)?;
        Ok(())
    }

    /// Write raw string with newline
    pub fn writeln(&mut self, content: &str) -> io::Result<()> {
        writeln!(self.writer, "{}", content)?;
        Ok(())
    }

    /// Get the inner writer
    pub fn into_inner(self) -> Box<dyn Write> {
        self.writer
    }
    
    /// Get a reference to the formatter for testing
    #[cfg(test)]
    pub fn formatter(&self) -> &dyn OutputFormatter {
        self.formatter.as_ref()
    }

    /// Check if colors should be used
    pub fn use_color(&self) -> bool {
        self.use_color
    }

    /// Check if emojis should be used
    pub fn use_emoji(&self) -> bool {
        self.use_emoji
    }

    /// Get the output format
    pub fn format(&self) -> &OutputFormat {
        &self.format
    }

    /// Get the verbosity level
    pub fn verbosity(&self) -> &VerbosityLevel {
        &self.verbosity
    }
}

// Implement Write trait to allow direct writing
impl Write for OutputContext {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.writer.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

