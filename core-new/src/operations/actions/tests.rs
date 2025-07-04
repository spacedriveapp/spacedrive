//! Tests for the Action System

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        context::CoreContext,
        operations::actions::{Action, registry::REGISTRY},
    };

    #[test]
    fn test_action_kind() {
        let action = Action::LibraryCreate {
            name: "Test Library".to_string(),
            path: None,
        };
        assert_eq!(action.kind(), "library.create");
    }

    #[test]
    fn test_action_description() {
        let action = Action::LibraryCreate {
            name: "Test Library".to_string(),
            path: None,
        };
        assert_eq!(action.description(), "Create library 'Test Library'");
    }

    #[test]
    fn test_action_targets_summary() {
        let action = Action::LibraryCreate {
            name: "Test Library".to_string(),
            path: Some("/path/to/library".into()),
        };
        let summary = action.targets_summary();
        assert_eq!(summary["name"], "Test Library");
        assert_eq!(summary["path"], "/path/to/library");
    }

    #[test]
    fn test_registry_has_handlers() {
        // Test that the registry has been populated
        assert!(REGISTRY.has_action("library.create"));
        assert!(REGISTRY.has_action("library.delete"));
        assert!(REGISTRY.has_action("file.copy"));
        assert!(REGISTRY.has_action("file.delete"));
        assert!(REGISTRY.has_action("location.add"));
        assert!(REGISTRY.has_action("location.remove"));
        assert!(REGISTRY.has_action("location.index"));
        
        // Test that unknown actions are not registered
        assert!(!REGISTRY.has_action("unknown.action"));
    }

    #[test]
    fn test_action_registry_get_handler() {
        let handler = REGISTRY.get("library.create");
        assert!(handler.is_some());
        
        let handler = REGISTRY.get("unknown.action");
        assert!(handler.is_none());
    }
}