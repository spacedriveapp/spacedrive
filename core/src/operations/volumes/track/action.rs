//! Track volume action
//!
//! This action tracks a volume within a library, allowing Spacedrive to monitor
//! and index files on the volume.

use crate::{
    infrastructure::actions::{error::ActionError, output::ActionOutput},
    volume::VolumeFingerprint,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Input for tracking a volume
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeTrackAction {
    /// The fingerprint of the volume to track
    pub fingerprint: VolumeFingerprint,
    
    /// The library ID to track the volume in
    pub library_id: Uuid,
    
    /// Optional name for the tracked volume
    pub name: Option<String>,
}

impl VolumeTrackAction {
    /// Execute the volume tracking action
    pub async fn execute(
        &self,
        core: &crate::Core,
    ) -> Result<ActionOutput, ActionError> {
        // Get the library
        let library = core
            .libraries
            .get_library(self.library_id)
            .await
            .ok_or_else(|| ActionError::InvalidInput("Library not found".to_string()))?;
            
        // Check if volume exists
        let volume = core
            .volumes
            .get_volume(&self.fingerprint)
            .await
            .ok_or_else(|| ActionError::InvalidInput("Volume not found".to_string()))?;
            
        // TODO: Implement actual volume tracking in library
        // For now, just verify the volume exists and is mounted
        if !volume.is_mounted {
            return Err(ActionError::InvalidInput(
                "Cannot track unmounted volume".to_string()
            ));
        }
        
        Ok(ActionOutput::VolumeTracked {
            fingerprint: self.fingerprint.clone(),
            library_id: self.library_id,
            volume_name: volume.name,
        })
    }
}