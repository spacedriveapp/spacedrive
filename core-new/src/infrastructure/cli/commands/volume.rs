//! Volume CLI commands

use clap::Subcommand;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Volume-related commands
#[derive(Debug, Clone, Subcommand, Serialize, Deserialize)]
pub enum VolumeCommands {
    /// List all volumes
    List,
    
    /// Show details for a specific volume
    Get {
        /// Volume fingerprint
        fingerprint: String,
    },
    
    /// Track a volume in a library
    Track {
        /// Library ID
        library_id: Uuid,
        
        /// Volume fingerprint
        fingerprint: String,
        
        /// Optional name for the tracked volume
        #[arg(short, long)]
        name: Option<String>,
    },
    
    /// Untrack a volume from a library
    Untrack {
        /// Library ID
        library_id: Uuid,
        
        /// Volume fingerprint
        fingerprint: String,
    },
    
    /// Run speed test on a volume
    SpeedTest {
        /// Volume fingerprint
        fingerprint: String,
    },
    
    /// Refresh volume list
    Refresh,
}