//! Section builder for complex multi-line outputs

use super::context::OutputContext;
use comfy_table::{Table, presets::UTF8_FULL};
use owo_colors::OwoColorize;
use std::io;

/// Line types for section building
#[derive(Debug)]
enum Line {
    Title(String),
    Status(String, String),
    Item(String, String),
    Text(String),
    Empty,
    Table(Table),
}

/// Fluent builder for output sections
pub struct OutputSection<'a> {
    context: &'a mut OutputContext,
    lines: Vec<Line>,
}

impl<'a> OutputSection<'a> {
    /// Create a new section builder
    pub fn new(context: &'a mut OutputContext) -> Self {
        Self {
            context,
            lines: Vec::new(),
        }
    }

    /// Add a title
    pub fn title(mut self, text: &str) -> Self {
        self.lines.push(Line::Title(text.to_string()));
        self
    }

    /// Add a status line (label: value)
    pub fn status(mut self, label: &str, value: &str) -> Self {
        self.lines.push(Line::Status(label.to_string(), value.to_string()));
        self
    }

    /// Add an item line (label: value with indentation)
    pub fn item(mut self, label: &str, value: &str) -> Self {
        self.lines.push(Line::Item(label.to_string(), value.to_string()));
        self
    }

    /// Add plain text
    pub fn text(mut self, text: &str) -> Self {
        self.lines.push(Line::Text(text.to_string()));
        self
    }

    /// Add an empty line
    pub fn empty_line(mut self) -> Self {
        self.lines.push(Line::Empty);
        self
    }

    /// Add a table
    pub fn table(mut self, table: Table) -> Self {
        self.lines.push(Line::Table(table));
        self
    }

    /// Add a help section
    pub fn help(self) -> HelpSection<'a> {
        HelpSection::new(self)
    }

    /// Render the section
    pub fn render(self) -> io::Result<()> {
        let use_color = self.context.use_color();
        let mut last_was_empty = false;

        for (i, line) in self.lines.iter().enumerate() {
            // Smart spacing: avoid duplicate empty lines
            if matches!(line, Line::Empty) {
                if !last_was_empty && i > 0 {
                    self.context.writeln("")?;
                }
                last_was_empty = true;
                continue;
            }
            last_was_empty = false;

            match line {
                Line::Title(text) => {
                    // Add spacing before title if not first line
                    if i > 0 {
                        self.context.writeln("")?;
                    }
                    if use_color {
                        self.context.writeln(&text.bold().cyan().to_string())?;
                    } else {
                        self.context.writeln(text)?;
                    }
                }
                Line::Status(label, value) => {
                    if use_color {
                        self.context.writeln(&format!("{}: {}", label, value.bold()))?;
                    } else {
                        self.context.writeln(&format!("{}: {}", label, value))?;
                    }
                }
                Line::Item(label, value) => {
                    if use_color {
                        self.context.writeln(&format!("  {}: {}", label, value.bold()))?;
                    } else {
                        self.context.writeln(&format!("  {}: {}", label, value))?;
                    }
                }
                Line::Text(text) => {
                    self.context.writeln(text)?;
                }
                Line::Table(table) => {
                    self.context.write(&table.to_string())?;
                }
                Line::Empty => {} // Handled above
            }
        }

        Ok(())
    }
}

/// Help section builder
pub struct HelpSection<'a> {
    parent: OutputSection<'a>,
    items: Vec<String>,
}

impl<'a> HelpSection<'a> {
    fn new(parent: OutputSection<'a>) -> Self {
        Self {
            parent,
            items: Vec::new(),
        }
    }

    /// Add a help item
    pub fn item(mut self, text: &str) -> Self {
        self.items.push(text.to_string());
        self
    }

    /// Finish help section and return to parent
    pub fn end(mut self) -> OutputSection<'a> {
        if !self.items.is_empty() {
            self.parent.lines.push(Line::Empty);

            if self.parent.context.use_emoji() {
                self.parent.lines.push(Line::Text("💡 Tips:".to_string()));
            } else {
                self.parent.lines.push(Line::Text("Tips:".to_string()));
            }

            for item in self.items {
                self.parent.lines.push(Line::Text(format!("   • {}", item)));
            }
        }
        self.parent
    }

    /// Render the help section (consumes self)
    pub fn render(self) -> io::Result<()> {
        self.end().render()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::cli::output::{CliOutput, OutputFormat};

    #[test]
    fn test_section_builder() {
        let (mut output, buffer) = CliOutput::test();

        output.section()
            .title("Test Section")
            .status("Status", "Active")
            .empty_line()
            .item("Item 1", "Value 1")
            .item("Item 2", "Value 2")
            .render()
            .unwrap();

        let result = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();
        assert!(result.contains("Test Section"));
        assert!(result.contains("Status: Active"));
        assert!(result.contains("Item 1: Value 1"));
    }

    #[test]
    fn test_help_section() {
        let (mut output, buffer) = CliOutput::test();

        output.section()
            .title("Commands")
            .help()
                .item("Use 'create' to make new items")
                .item("Use 'delete' to remove items")
            .render()
            .unwrap();

        let result = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();
        assert!(result.contains("Commands"));
        assert!(result.contains("Tips:"));
        assert!(result.contains("• Use 'create' to make new items"));
    }
}