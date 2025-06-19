//! Configuration migration system

use anyhow::Result;

/// Trait for versioned configuration migration
pub trait Migrate {
    /// Get the current version of this configuration
    fn current_version(&self) -> u32;
    
    /// Get the target version this configuration should be migrated to
    fn target_version() -> u32;
    
    /// Apply migrations to bring configuration to target version
    fn migrate(&mut self) -> Result<()>;
    
    /// Check if migration is needed
    fn needs_migration(&self) -> bool {
        self.current_version() < Self::target_version()
    }
}