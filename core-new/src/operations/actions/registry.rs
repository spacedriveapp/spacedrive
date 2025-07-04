//! Action registry for automatic handler discovery

use super::{handler::ActionHandler, error::{ActionError, ActionResult}};
use inventory;
use once_cell::sync::Lazy;
use std::{collections::HashMap, sync::Arc};
use tracing::info;

/// Registration struct for action handlers
pub struct ActionRegistration {
    pub name: &'static str,
    pub create_fn: fn() -> Box<dyn ActionHandler>,
}

// Inventory for auto-registration
inventory::collect!(ActionRegistration);

/// Global action registry
pub struct ActionRegistry {
    handlers: HashMap<&'static str, Box<dyn ActionHandler>>,
}

impl ActionRegistry {
    /// Create a new registry and discover all handlers
    pub fn new() -> Self {
        let mut handlers = HashMap::new();
        
        // Collect all registered action handlers
        for registration in inventory::iter::<ActionRegistration> {
            info!("Registered action handler: {}", registration.name);
            handlers.insert(registration.name, (registration.create_fn)());
        }
        
        info!("Discovered {} action handler types", handlers.len());
        
        Self { handlers }
    }
    
    /// Get a handler for the given action kind
    pub fn get(&self, action_kind: &str) -> Option<&dyn ActionHandler> {
        self.handlers.get(action_kind).map(|h| h.as_ref())
    }
    
    /// Get all registered action kinds
    pub fn action_kinds(&self) -> Vec<&'static str> {
        self.handlers.keys().copied().collect()
    }
    
    /// Check if an action kind is registered
    pub fn has_action(&self, action_kind: &str) -> bool {
        self.handlers.contains_key(action_kind)
    }
}

/// Global registry instance
pub static REGISTRY: Lazy<ActionRegistry> = Lazy::new(ActionRegistry::new);

/// Helper macro for registering action handlers
#[macro_export]
macro_rules! register_action_handler {
    ($handler_type:ty, $action_kind:expr) => {
        inventory::submit! {
            $crate::operations::actions::registry::ActionRegistration {
                name: $action_kind,
                create_fn: || Box::new(<$handler_type>::new()),
            }
        }
    };
}