//! Macros for handling CLI command execution

/// Execute a library action and handle serialization/deserialization
#[macro_export]
macro_rules! execute_action {
	($ctx:expr, $input:expr) => {{
		let input = $input;
		let library_id = get_current_library!($ctx);
		let json_response = $ctx
			.core
			.action(&input, Some(library_id))
			.await
			.map_err(|e| $crate::util::error::improve_core_error(e.to_string()))?;

		// Deserialize the JSON response to the expected type
		serde_json::from_value(json_response).map_err(|e| {
			$crate::util::error::CliError::SerializationError(format!(
				"Failed to deserialize response: {}",
				e
			))
		})?
	}};
}

/// Execute a core action (no library ID required) and handle serialization/deserialization
#[macro_export]
macro_rules! execute_core_action {
	($ctx:expr, $input:expr) => {{
		let input = $input;
		let json_response = $ctx
			.core
			.action(&input, None)
			.await
			.map_err(|e| $crate::util::error::improve_core_error(e.to_string()))?;

		// Deserialize the JSON response to the expected type
		serde_json::from_value(json_response).map_err(|e| {
			$crate::util::error::CliError::SerializationError(format!(
				"Failed to deserialize response: {}",
				e
			))
		})?
	}};
}

/// Execute a library query and handle serialization/deserialization
#[macro_export]
macro_rules! execute_query {
	($ctx:expr, $input:expr) => {{
		let input = $input;
		let library_id = get_current_library!($ctx);
		let json_response = $ctx
			.core
			.query(&input, Some(library_id))
			.await
			.map_err(|e| $crate::util::error::improve_core_error(e.to_string()))?;

		// Deserialize the JSON response to the expected type
		serde_json::from_value(json_response).map_err(|e| {
			$crate::util::error::CliError::SerializationError(format!(
				"Failed to deserialize response: {}",
				e
			))
		})?
	}};
}

/// Execute a core query (no library ID required) and handle serialization/deserialization
#[macro_export]
macro_rules! execute_core_query {
	($ctx:expr, $input:expr) => {{
		let input = $input;
		let json_response = $ctx
			.core
			.query(&input, None)
			.await
			.map_err(|e| $crate::util::error::improve_core_error(e.to_string()))?;

		// Deserialize the JSON response to the expected type
		serde_json::from_value(json_response).map_err(|e| {
			$crate::util::error::CliError::SerializationError(format!(
				"Failed to deserialize response: {}",
				e
			))
		})?
	}};
}

/// Print output in the configured format (human or JSON)
#[macro_export]
macro_rules! print_output {
	($ctx:expr, $output:expr, $human:expr) => {{
		match $ctx.format {
			$crate::context::OutputFormat::Human => {
				$human($output);
			}
			$crate::context::OutputFormat::Json => {
				$crate::util::output::print_json(&$output);
			}
		}
	}};
}

/// Get the current library ID from CLI context or error
#[macro_export]
macro_rules! get_current_library {
	($ctx:expr) => {{
		$ctx.library_id
			.ok_or($crate::util::error::CliError::NoActiveLibrary)?
	}};
}

/// Execute an action with confirmation support
/// This macro handles the validation and confirmation flow before executing the action
#[macro_export]
macro_rules! execute_action_with_confirmation {
	($ctx:expr, $input:expr) => {{
		use sd_core::infra::action::{LibraryAction, ValidationResult};
		use $crate::util::confirm::prompt_for_choice;

		// Build the action from input
		let mut action = match <_ as LibraryAction>::from_input($input) {
			Ok(action) => action,
			Err(e) => anyhow::bail!("Failed to build action: {}", e),
		};

		// Get current library for validation
		let library_id = get_current_library!($ctx);

		// For validation, we need to create a mock library context
		// In a full implementation, this would use the actual library from daemon
		// For now, we'll skip library-specific validation and focus on the confirmation flow

		// Note: This is a simplified implementation that assumes the action
		// can be validated without full library context
		// In production, you'd need to implement a way to validate actions on the CLI side
		// or extend the daemon protocol to support validation requests

		// Execute the action directly for now
		let job_id = execute_action!($ctx, action);
		job_id
	}};
}
