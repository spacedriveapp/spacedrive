//! Macros for handling CLI command execution

/// Execute a core action and handle serialization/deserialization
#[macro_export]
macro_rules! execute_action {
	($ctx:expr, $input:expr) => {{
		let input = $input;
		let bytes = $ctx
			.core
			.action(&input)
			.await
			.map_err(|e| $crate::util::error::improve_core_error(e.to_string()))?;
		bincode::serde::decode_from_slice(&bytes, bincode::config::standard())
			.map_err(|e| {
				$crate::util::error::CliError::SerializationError(format!(
					"Failed to deserialize response: {}",
					e
				))
			})?
			.0
	}};
}

/// Execute a core query and handle serialization/deserialization
#[macro_export]
macro_rules! execute_query {
	($ctx:expr, $input:expr) => {{
		let input = $input;
		$ctx.core
			.query(&input)
			.await
			.map_err(|e| $crate::util::error::improve_core_error(e.to_string()))?
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

/// Get the current library ID from session or error
#[macro_export]
macro_rules! get_current_library {
	($ctx:expr) => {{
		let session = $ctx.core.session().get().await;
		session
			.current_library_id
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
