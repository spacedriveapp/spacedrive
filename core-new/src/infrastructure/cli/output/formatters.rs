//! Output formatters for different output modes

use super::context::OutputContext;
use super::messages::*;
use super::{BULB, CHART, DEVICE, ERROR, FOLDER, INFO, LIBRARY, NETWORK, ROCKET, STOP, SUCCESS, TRASH, WARNING};
use owo_colors::OwoColorize;
use serde_json::json;

/// Core output trait - implemented for each format
pub trait OutputFormatter: Send {
    /// Format a message
    fn format(&self, message: &Message, context: &OutputContext) -> String;
    
    /// Format an error message
    fn format_error(&self, message: &Message, context: &OutputContext) -> String {
        self.format(message, context)
    }
}

/// Human-readable formatter with colors and emojis
pub struct HumanFormatter {
    use_color: bool,
    use_emoji: bool,
}

impl HumanFormatter {
    pub fn new(use_color: bool, use_emoji: bool) -> Self {
        Self { use_color, use_emoji }
    }

    fn emoji(&self, emoji: console::Emoji) -> String {
        if self.use_emoji {
            emoji.to_string()
        } else {
            "".to_string()
        }
    }

    fn colored(&self, text: impl std::fmt::Display) -> ColorHelper {
        ColorHelper {
            text: text.to_string(),
            use_color: self.use_color,
        }
    }
}

struct ColorHelper {
    text: String,
    use_color: bool,
}

impl ColorHelper {
    fn green(self) -> String {
        if self.use_color {
            self.text.green().to_string()
        } else {
            self.text
        }
    }

    fn red(self) -> String {
        if self.use_color {
            self.text.red().to_string()
        } else {
            self.text
        }
    }

    fn yellow(self) -> String {
        if self.use_color {
            self.text.yellow().to_string()
        } else {
            self.text
        }
    }

    fn bright_cyan(self) -> String {
        if self.use_color {
            self.text.bright_cyan().to_string()
        } else {
            self.text
        }
    }

    fn bright_yellow(self) -> String {
        if self.use_color {
            self.text.bright_yellow().to_string()
        } else {
            self.text
        }
    }

    fn bright_blue(self) -> String {
        if self.use_color {
            self.text.bright_blue().to_string()
        } else {
            self.text
        }
    }

    fn bright_green(self) -> String {
        if self.use_color {
            self.text.bright_green().to_string()
        } else {
            self.text
        }
    }

    fn bright_red(self) -> String {
        if self.use_color {
            self.text.bright_red().to_string()
        } else {
            self.text
        }
    }

    fn dimmed(self) -> String {
        if self.use_color {
            self.text.dimmed().to_string()
        } else {
            self.text
        }
    }

    fn bold(self) -> String {
        if self.use_color {
            self.text.bold().to_string()
        } else {
            self.text
        }
    }

    fn cyan(self) -> String {
        if self.use_color {
            self.text.cyan().to_string()
        } else {
            self.text
        }
    }
}

impl OutputFormatter for HumanFormatter {
    fn format(&self, message: &Message, _context: &OutputContext) -> String {
        match message {
            Message::Success(text) => {
                format!("{}{}", self.emoji(SUCCESS), self.colored(text).green())
            }
            Message::Error(text) => {
                format!("{}{}", self.emoji(ERROR), self.colored(text).red())
            }
            Message::Warning(text) => {
                format!("{}{}", self.emoji(WARNING), self.colored(text).yellow())
            }
            Message::Info(text) => {
                format!("{}{}", self.emoji(INFO), text)
            }
            Message::Debug(text) => {
                format!("[DEBUG] {}", self.colored(text).dimmed())
            }

            // Library messages
            Message::LibraryCreated { name, id, path } => {
                format!(
                    "{}Library '{}' created successfully!\n   ID: {}\n   Path: {}\n   Status: {}",
                    self.emoji(SUCCESS),
                    self.colored(name).bright_cyan(),
                    self.colored(id.to_string()).bright_yellow(),
                    self.colored(path.display()).bright_blue(),
                    self.colored("Active").bright_green()
                )
            }
            Message::LibraryList { libraries } => {
                if libraries.is_empty() {
                    format!("{}No libraries found. Create one with: spacedrive library create <name>", self.emoji(LIBRARY))
                } else {
                    let mut output = format!("{}Libraries:\n", self.emoji(LIBRARY));
                    for lib in libraries {
                        output.push_str(&format!(
                            "  {} {} - {}\n",
                            self.colored(&lib.id.to_string()[..8]).dimmed(),
                            self.colored(&lib.name).bright_cyan(),
                            lib.path.display()
                        ));
                    }
                    output.trim_end().to_string()
                }
            }
            Message::CurrentLibrary { library } => {
                match library {
                    Some(lib) => format!(
                        "{}Current library: {}\n   ID: {}\n   Path: {}",
                        self.emoji(LIBRARY),
                        self.colored(&lib.name).bright_cyan(),
                        self.colored(lib.id.to_string()).bright_yellow(),
                        self.colored(lib.path.display()).bright_blue()
                    ),
                    None => format!("{}No current library selected", self.emoji(WARNING)),
                }
            }
            Message::NoLibrariesFound => {
                format!("{}No libraries found. Create one with: spacedrive library create <name>", self.emoji(LIBRARY))
            }

            // Location messages
            Message::LocationAdded { path, id } => {
                format!(
                    "{}Location added successfully!\n   Path: {}\n   ID: {}",
                    self.emoji(SUCCESS),
                    self.colored(path.display()).bright_blue(),
                    self.colored(id.to_string()).bright_yellow()
                )
            }
            Message::LocationList { locations } => {
                if locations.is_empty() {
                    format!("{}No locations found", self.emoji(FOLDER))
                } else {
                    let mut output = format!("{}Locations:\n", self.emoji(FOLDER));
                    for loc in locations {
                        output.push_str(&format!(
                            "  {} - {} files\n",
                            self.colored(loc.path.display()).bright_blue(),
                            loc.indexed_files
                        ));
                    }
                    output.trim_end().to_string()
                }
            }

            // Daemon messages
            Message::DaemonStarting { instance } => {
                format!("{}Starting Spacedrive daemon ({})", self.emoji(ROCKET), instance)
            }
            Message::DaemonStopping { instance } => {
                format!("{}Stopping Spacedrive daemon instance '{}'...", self.emoji(STOP), instance)
            }
            Message::DaemonStopped { instance } => {
                format!("{}Spacedrive daemon instance '{}' stopped", self.emoji(SUCCESS), instance)
            }
            Message::DaemonStarted { instance, pid, socket_path } => {
                format!(
                    "{}Daemon started successfully ({})\n   PID: {}\n   Socket: {}",
                    self.emoji(SUCCESS),
                    instance,
                    self.colored(pid).bright_yellow(),
                    self.colored(socket_path.display()).bright_blue()
                )
            }
            Message::DaemonNotRunning { instance } => {
                format!(
                    "{}Spacedrive daemon instance '{}' is not running\n   Start it with: spacedrive start",
                    self.emoji(ERROR),
                    instance
                )
            }
            Message::DaemonStatus { version, uptime, instance, networking_enabled, libraries } => {
                let mut output = format!(
                    "{}Spacedrive Daemon Status\n",
                    self.emoji(CHART)
                );
                output.push_str(&format!("   Version: {}\n", self.colored(version).bright_green()));
                output.push_str(&format!("   Instance: {}\n", instance));
                output.push_str(&format!("   Uptime: {} seconds\n", uptime));
                output.push_str(&format!("   Networking: {}\n", 
                    if *networking_enabled { 
                        self.colored("Enabled").green()
                    } else { 
                        self.colored("Disabled").red()
                    }
                ));
                output.push_str(&format!("   Libraries: {}", libraries.len()));
                output
            }

            // Network messages
            Message::NetworkingInitialized => {
                format!("{}Networking initialized successfully", self.emoji(SUCCESS))
            }
            Message::NetworkingStarted => {
                format!("{}Networking service started", self.emoji(NETWORK))
            }
            Message::DevicesList { devices } => {
                if devices.is_empty() {
                    format!("{}No devices found", self.emoji(DEVICE))
                } else {
                    let mut output = format!("{}Discovered devices:\n", self.emoji(DEVICE));
                    for device in devices {
                        let status_str = format!("{:?}", device.status);
                        let status_colored = match device.status {
                            DeviceStatus::Online => self.colored(status_str).green(),
                            DeviceStatus::Paired => self.colored(status_str).bright_green(),
                            DeviceStatus::Offline => self.colored(status_str).red(),
                            DeviceStatus::Discovered => self.colored(status_str).yellow(),
                        };
                        let id_display = if device.id.len() >= 8 {
                            &device.id[..8]
                        } else {
                            &device.id
                        };
                        output.push_str(&format!(
                            "  {} {} - {}\n",
                            self.colored(id_display).dimmed(),
                            self.colored(&device.name).bright_cyan(),
                            status_colored
                        ));
                    }
                    output.trim_end().to_string()
                }
            }
            Message::PairingCodeGenerated { code } => {
                format!(
                    "{}Pairing code generated:\n\n   {}\n\n{}Share this code with the device you want to pair",
                    self.emoji(SUCCESS),
                    self.colored(code).bright_green().bold(),
                    self.emoji(BULB)
                )
            }
            Message::PairingSuccess { device_name, device_id } => {
                format!(
                    "{}Successfully paired with {} ({})",
                    self.emoji(SUCCESS),
                    self.colored(device_name).bright_cyan(),
                    self.colored(if device_id.len() >= 8 { &device_id[..8] } else { device_id }).dimmed()
                )
            }

            // Job messages
            Message::JobStarted { name, .. } => {
                format!("{}Job started: {}", self.emoji(ROCKET), self.colored(name).bright_cyan())
            }
            Message::JobCompleted { name, duration, .. } => {
                format!(
                    "{}Job completed: {} ({}s)",
                    self.emoji(SUCCESS),
                    self.colored(name).bright_cyan(),
                    duration
                )
            }
            Message::JobFailed { name, error, .. } => {
                format!(
                    "{}Job failed: {}\n   Error: {}",
                    self.emoji(ERROR),
                    self.colored(name).bright_cyan(),
                    self.colored(error).red()
                )
            }

            // File operation messages
            Message::FileCopied { source, destination } => {
                format!(
                    "{}Copied {} → {}",
                    self.emoji(SUCCESS),
                    self.colored(source.display()).bright_blue(),
                    self.colored(destination.display()).bright_green()
                )
            }
            Message::FileDeleted { path } => {
                format!(
                    "{}Deleted {}",
                    self.emoji(TRASH),
                    self.colored(path.display()).bright_red()
                )
            }

            // Help messages
            Message::HelpText { lines } => {
                let mut output = format!("{}Tips:\n", self.emoji(BULB));
                for line in lines {
                    output.push_str(&format!("   • {}\n", line));
                }
                output.trim_end().to_string()
            }

            // Progress messages
            Message::IndexingProgress { current, total, location } => {
                format!(
                    "Indexing {}: {}/{} files",
                    self.colored(location).bright_blue(),
                    current,
                    total
                )
            }
            Message::CopyProgress { current, total, current_file } => {
                match current_file {
                    Some(file) => format!(
                        "Copying {}: {}/{} files",
                        self.colored(file).bright_blue(),
                        current,
                        total
                    ),
                    None => format!("Copying: {}/{} files", current, total),
                }
            }
            Message::ValidationProgress { current, total } => {
                format!("Validating: {}/{} files", current, total)
            }

            // Fallback for other messages
            _ => format!("{:?}", message),
        }
    }

    fn format_error(&self, message: &Message, context: &OutputContext) -> String {
        // Errors are always formatted with error styling
        match message {
            Message::Error(text) => {
                format!("{}{}", self.emoji(ERROR), self.colored(text).red())
            }
            _ => self.format(message, context),
        }
    }
}

/// JSON formatter for machine-readable output
pub struct JsonFormatter;

impl OutputFormatter for JsonFormatter {
    fn format(&self, message: &Message, _context: &OutputContext) -> String {
        // Convert message to JSON
        match message {
            Message::Success(text) => json!({
                "type": "success",
                "message": text
            }).to_string(),
            Message::Error(text) => json!({
                "type": "error",
                "message": text
            }).to_string(),
            Message::LibraryCreated { name, id, path } => json!({
                "type": "library_created",
                "success": true,
                "data": {
                    "name": name,
                    "id": id.to_string(),
                    "path": path.to_string_lossy()
                }
            }).to_string(),
            Message::LibraryList { libraries } => json!({
                "type": "library_list",
                "data": libraries.iter().map(|lib| json!({
                    "id": lib.id.to_string(),
                    "name": lib.name,
                    "path": lib.path.to_string_lossy()
                })).collect::<Vec<_>>()
            }).to_string(),
            Message::DevicesList { devices } => json!({
                "type": "devices_list",
                "data": devices.iter().map(|dev| json!({
                    "id": dev.id,
                    "name": dev.name,
                    "status": format!("{:?}", dev.status).to_lowercase()
                })).collect::<Vec<_>>()
            }).to_string(),
            _ => {
                // Fallback to serializing the entire message
                serde_json::to_string(message).unwrap_or_else(|_| {
                    json!({
                        "type": "unknown",
                        "message": format!("{:?}", message)
                    }).to_string()
                })
            }
        }
    }
}