//! Watch configuration types
//!
//! Configuration for how paths should be watched and events filtered.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for watching a path
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchConfig {
	/// Whether to watch recursively (all subdirectories) or shallow (immediate children only)
	pub recursive: bool,
	/// Rules for filtering events
	pub filters: EventFilters,
}

impl Default for WatchConfig {
	fn default() -> Self {
		Self {
			recursive: true,
			filters: EventFilters::default(),
		}
	}
}

impl WatchConfig {
	/// Create a recursive watch configuration with default filters
	pub fn recursive() -> Self {
		Self {
			recursive: true,
			filters: EventFilters::default(),
		}
	}

	/// Create a shallow (non-recursive) watch configuration with default filters
	pub fn shallow() -> Self {
		Self {
			recursive: false,
			filters: EventFilters::default(),
		}
	}

	/// Set whether to watch recursively
	pub fn with_recursive(mut self, recursive: bool) -> Self {
		self.recursive = recursive;
		self
	}

	/// Set the event filters
	pub fn with_filters(mut self, filters: EventFilters) -> Self {
		self.filters = filters;
		self
	}
}

/// Filters for which events to emit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventFilters {
	/// Skip hidden files (starting with .)
	pub skip_hidden: bool,
	/// Skip system files (.DS_Store, Thumbs.db, etc.)
	pub skip_system_files: bool,
	/// Skip temporary files (.tmp, .temp, ~, .swp)
	pub skip_temp_files: bool,
	/// Custom patterns to skip (glob patterns)
	pub skip_patterns: Vec<String>,
	/// Keep these dotfiles even if skip_hidden is true
	pub important_dotfiles: Vec<String>,
}

impl Default for EventFilters {
	fn default() -> Self {
		Self {
			skip_hidden: true,
			skip_system_files: true,
			skip_temp_files: true,
			skip_patterns: Vec::new(),
			important_dotfiles: vec![
				".gitignore".to_string(),
				".gitkeep".to_string(),
				".gitattributes".to_string(),
				".editorconfig".to_string(),
				".env".to_string(),
				".env.local".to_string(),
				".nvmrc".to_string(),
				".node-version".to_string(),
				".python-version".to_string(),
				".dockerignore".to_string(),
				".eslintrc".to_string(),
				".prettierrc".to_string(),
			],
		}
	}
}

impl EventFilters {
	/// Create filters that allow all events (no filtering)
	pub fn allow_all() -> Self {
		Self {
			skip_hidden: false,
			skip_system_files: false,
			skip_temp_files: false,
			skip_patterns: Vec::new(),
			important_dotfiles: Vec::new(),
		}
	}

	/// Check if a path should be filtered out
	pub fn should_skip(&self, path: &std::path::Path) -> bool {
		let path_str = path.to_string_lossy();

		// Check temp files
		if self.skip_temp_files
			&& (path_str.contains(".tmp")
				|| path_str.contains(".temp")
				|| path_str.ends_with("~")
				|| path_str.ends_with(".swp"))
		{
			return true;
		}

		// Check system files
		if self.skip_system_files
			&& (path_str.contains(".DS_Store") || path_str.contains("Thumbs.db"))
		{
			return true;
		}

		// Check hidden files
		if self.skip_hidden {
			if let Some(file_name) = path.file_name() {
				let name = file_name.to_string_lossy();
				if name.starts_with('.') {
					// Check if it's an important dotfile
					let is_important = self
						.important_dotfiles
						.iter()
						.any(|d| d.as_str() == name.as_ref());
					if !is_important {
						return true;
					}
				}
			}
		}

		// Check custom skip patterns
		for pattern in &self.skip_patterns {
			if path_str.contains(pattern) {
				return true;
			}
		}

		false
	}
}

/// Global watcher configuration
#[derive(Debug, Clone)]
pub struct WatcherConfig {
	/// Size of the event channel buffer
	pub event_buffer_size: usize,
	/// Platform-specific tick interval for buffered event eviction
	pub tick_interval: Duration,
	/// Debounce duration for rapid events
	pub debounce_duration: Duration,
	/// Enable detailed debug logging
	pub debug_mode: bool,
}

impl Default for WatcherConfig {
	fn default() -> Self {
		Self {
			event_buffer_size: 100_000,
			tick_interval: Duration::from_millis(100),
			debounce_duration: Duration::from_millis(100),
			debug_mode: false,
		}
	}
}

impl WatcherConfig {
	/// Create a new watcher configuration
	pub fn new() -> Self {
		Self::default()
	}

	/// Set the event buffer size
	pub fn with_buffer_size(mut self, size: usize) -> Self {
		self.event_buffer_size = size;
		self
	}

	/// Set the tick interval
	pub fn with_tick_interval(mut self, interval: Duration) -> Self {
		self.tick_interval = interval;
		self
	}

	/// Set the debounce duration
	pub fn with_debounce(mut self, duration: Duration) -> Self {
		self.debounce_duration = duration;
		self
	}

	/// Enable debug mode
	pub fn with_debug(mut self, debug: bool) -> Self {
		self.debug_mode = debug;
		self
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::path::PathBuf;

	#[test]
	fn test_default_filters() {
		let filters = EventFilters::default();

		// Should skip system files
		assert!(filters.should_skip(&PathBuf::from("/test/.DS_Store")));
		assert!(filters.should_skip(&PathBuf::from("/test/Thumbs.db")));

		// Should skip temp files
		assert!(filters.should_skip(&PathBuf::from("/test/file.tmp")));
		assert!(filters.should_skip(&PathBuf::from("/test/file~")));

		// Should skip hidden files
		assert!(filters.should_skip(&PathBuf::from("/test/.hidden")));

		// Should NOT skip important dotfiles
		assert!(!filters.should_skip(&PathBuf::from("/test/.gitignore")));
		assert!(!filters.should_skip(&PathBuf::from("/test/.env")));

		// Should NOT skip normal files
		assert!(!filters.should_skip(&PathBuf::from("/test/file.txt")));
	}

	#[test]
	fn test_allow_all_filters() {
		let filters = EventFilters::allow_all();

		// Should not skip anything
		assert!(!filters.should_skip(&PathBuf::from("/test/.DS_Store")));
		assert!(!filters.should_skip(&PathBuf::from("/test/.hidden")));
		assert!(!filters.should_skip(&PathBuf::from("/test/file.tmp")));
	}

	#[test]
	fn test_watch_config() {
		let config = WatchConfig::recursive();
		assert!(config.recursive);

		let config = WatchConfig::shallow();
		assert!(!config.recursive);
	}
}

