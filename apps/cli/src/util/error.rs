use std::fmt;

/// Custom error type for CLI operations
#[derive(Debug)]
pub enum CliError {
	/// No active library selected
	NoActiveLibrary,
	/// Library not found
	LibraryNotFound(uuid::Uuid),
	/// Location not found
	LocationNotFound(uuid::Uuid),
	/// Multiple libraries exist but no specific one selected
	MultipleLibraries,
	/// Daemon is not running
	DaemonNotRunning,
	/// Core operation failed
	CoreError(String),
	/// Serialization/deserialization error
	SerializationError(String),
	/// Other error
	Other(String),
}

impl fmt::Display for CliError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
            Self::NoActiveLibrary => write!(f, "No active library selected"),
            Self::LibraryNotFound(id) => write!(f, "Library not found: {}", id),
            Self::LocationNotFound(id) => write!(f, "Location not found: {}", id),
            Self::MultipleLibraries => write!(
                f,
                "Multiple libraries exist. Please specify one with --library or switch to it with 'library switch'"
            ),
            Self::DaemonNotRunning => {
                write!(f, "🚫 Spacedrive daemon is not running\n\n")?;
                write!(f, "💡 To start the daemon, run:\n")?;
                write!(f, "   sd start\n\n")?;
                write!(f, "   Or start with networking enabled:\n")?;
                write!(f, "   sd start --enable-networking")
            },
            Self::CoreError(msg) => write!(f, "Core operation failed: {}", msg),
            Self::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            Self::Other(msg) => write!(f, "{}", msg),
        }
	}
}

impl std::error::Error for CliError {}

impl From<anyhow::Error> for CliError {
	fn from(err: anyhow::Error) -> Self {
		Self::Other(err.to_string())
	}
}

impl From<bincode::error::DecodeError> for CliError {
	fn from(err: bincode::error::DecodeError) -> Self {
		Self::SerializationError(err.to_string())
	}
}

/// Result type for CLI operations
pub type CliResult<T> = Result<T, CliError>;

/// Check if an error message indicates the daemon is not running
pub fn is_daemon_connection_error(error_msg: &str) -> bool {
    error_msg.contains("Failed to connect to daemon socket")
        || error_msg.contains("Connection refused")
        || error_msg.contains("No such file or directory")
        || error_msg.contains("daemon socket")
}

/// Convert a core error to a more user-friendly CLI error
pub fn improve_core_error(error_msg: String) -> CliError {
    if is_daemon_connection_error(&error_msg) {
        CliError::DaemonNotRunning
    } else {
        CliError::CoreError(error_msg)
    }
}
