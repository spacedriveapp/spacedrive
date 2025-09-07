//! Untrack volume action
//!
//! This action removes volume tracking from a library.

use crate::{
    infra::actions::{error::ActionError, output::ActionOutput},
    volume::VolumeFingerprint,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Input for untracking a volume
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeUntrackAction {
    /// The fingerprint of the volume to untrack
    pub fingerprint: VolumeFingerprint,

    /// The library ID to untrack the volume from
    pub library_id: Uuid,
}

impl VolumeUntrackAction {
    /// Execute the volume untracking action
    pub async fn execute(
        &self,
        core: &crate::Core,
    ) -> Result<ActionOutput, ActionError> {
        // Get the library
        let _library = core
            .libraries
            .get_library(self.library_id)
            .await
            .ok_or_else(|| ActionError::InvalidInput("Library not found".to_string()))?;

        // TODO: Implement actual volume untracking from library
        // For now, just verify the library exists

        Ok(ActionOutput::VolumeUntracked {
            fingerprint: self.fingerprint.clone(),
            library_id: self.library_id,
        })
    }
}