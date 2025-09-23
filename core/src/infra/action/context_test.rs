//! Tests for action context functionality

#[cfg(test)]
mod tests {
    use super::super::context::{ActionContext, ActionContextProvider, sanitize_action_input};
    use serde_json::json;

    // Mock action for testing
    struct MockAction {
        input: MockInput,
    }

    #[derive(serde::Serialize)]
    struct MockInput {
        path: String,
        name: Option<String>,
    }

    impl ActionContextProvider for MockAction {
        fn create_action_context(&self) -> ActionContext {
            ActionContext::new(
                Self::action_type_name(),
                sanitize_action_input(&self.input),
                json!({
                    "operation": "mock_operation",
                    "trigger": "test"
                }),
            )
        }

        fn action_type_name() -> &'static str {
            "test.mock"
        }
    }

    #[test]
    fn test_action_context_creation() {
        let action = MockAction {
            input: MockInput {
                path: "/test/path".to_string(),
                name: Some("Test".to_string()),
            },
        };

        let context = action.create_action_context();

        assert_eq!(context.action_type, "test.mock");
        assert!(context.initiated_by.is_none());

        // Check sanitized input
        let expected_input = json!({
            "path": "/test/path",
            "name": "Test"
        });
        assert_eq!(context.action_input, expected_input);

        // Check context data
        let expected_context = json!({
            "operation": "mock_operation",
            "trigger": "test"
        });
        assert_eq!(context.context, expected_context);
    }

    #[test]
    fn test_action_context_with_user() {
        let action = MockAction {
            input: MockInput {
                path: "/test/path".to_string(),
                name: None,
            },
        };

        let context = action
            .create_action_context()
            .with_initiated_by("test_user");

        assert_eq!(context.action_type, "test.mock");
        assert_eq!(context.initiated_by, Some("test_user".to_string()));
    }

    #[test]
    fn test_sanitize_action_input() {
        let input = MockInput {
            path: "/sensitive/path".to_string(),
            name: Some("Secret".to_string()),
        };

        let sanitized = sanitize_action_input(&input);

        assert_eq!(
            sanitized,
            json!({
                "path": "/sensitive/path",
                "name": "Secret"
            })
        );
    }
}

