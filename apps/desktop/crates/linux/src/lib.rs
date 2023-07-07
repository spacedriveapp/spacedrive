#![cfg(target_os = "linux")]

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error(transparent)]
	Io(#[from] std::io::Error),
	#[error(transparent)]
	Xdg(#[from] xdg::BaseDirectoriesError),
	#[error("no handlers found for '{0}'")]
	NotFound(String),
	#[error("bad Desktop Entry exec line: {0}")]
	InvalidExec(String),
	#[error("malformed desktop entry at {0}")]
	BadEntry(std::path::PathBuf),
	#[error("Please specify the default terminal with handlr set x-scheme-handler/terminal")]
	NoTerminal,
	#[error("Bad path: {0}")]
	BadPath(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

mod desktop_entry;
mod env;
mod handler;
mod system;

pub use desktop_entry::{DesktopEntry, Mode as ExecMode};
pub use env::normalize_environment;
pub use handler::{Handler, HandlerType};
pub use system::SystemApps;
