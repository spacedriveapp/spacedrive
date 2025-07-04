//! Action handler trait and related types

use super::{Action, error::ActionResult, receipt::ActionReceipt};
use crate::context::CoreContext;
use async_trait::async_trait;
use std::sync::Arc;

/// Trait that all action handlers must implement
#[async_trait]
pub trait ActionHandler: Send + Sync {
    /// Execute the action and return a receipt
    async fn execute(
        &self,
        context: Arc<CoreContext>,
        action: Action,
    ) -> ActionResult<ActionReceipt>;
    
    /// Validate the action before execution (optional)
    async fn validate(
        &self,
        _context: Arc<CoreContext>,
        _action: &Action,
    ) -> ActionResult<()> {
        Ok(())
    }
    
    /// Check if this handler can handle the given action
    fn can_handle(&self, action: &Action) -> bool;
    
    /// Get the action kinds this handler supports
    fn supported_actions() -> &'static [&'static str]
    where
        Self: Sized;
}