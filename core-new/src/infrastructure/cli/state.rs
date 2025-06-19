use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliState {
    /// Currently selected library ID
    pub current_library_id: Option<Uuid>,
    
    /// Last used library path
    pub last_library_path: Option<PathBuf>,
    
    /// Recent commands for history
    pub command_history: Vec<String>,
    
    /// Maximum history size
    #[serde(default = "default_history_size")]
    pub max_history: usize,
}

fn default_history_size() -> usize {
    100
}

impl Default for CliState {
    fn default() -> Self {
        Self {
            current_library_id: None,
            last_library_path: None,
            command_history: Vec::new(),
            max_history: default_history_size(),
        }
    }
}

impl CliState {
    pub fn load(data_dir: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let state_file = data_dir.join("cli_state.json");
        
        if state_file.exists() {
            let content = std::fs::read_to_string(&state_file)?;
            let state: Self = serde_json::from_str(&content)?;
            Ok(state)
        } else {
            Ok(Self::default())
        }
    }
    
    pub fn save(&self, data_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let state_file = data_dir.join("cli_state.json");
        
        // Ensure directory exists
        if let Some(parent) = state_file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&state_file, content)?;
        
        Ok(())
    }
    
    pub fn add_to_history(&mut self, command: String) {
        self.command_history.push(command);
        
        // Trim history if it exceeds max size
        if self.command_history.len() > self.max_history {
            self.command_history.remove(0);
        }
    }
    
    pub fn set_current_library(&mut self, library_id: Uuid, library_path: PathBuf) {
        self.current_library_id = Some(library_id);
        self.last_library_path = Some(library_path);
    }
    
    pub fn has_current_library(&self) -> bool {
        self.current_library_id.is_some()
    }
}