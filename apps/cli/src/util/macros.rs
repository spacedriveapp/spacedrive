//! Macros for handling CLI command execution

/// Execute a library action and handle serialization/deserialization
#[macro_export]
macro_rules! execute_action {
	($ctx:expr, $input:expr) => {{
		let input = $input;
		// A device-targeted request names a library on the target, whose ids
		// this client does not know; the target resolves its own.
		let library_id = match $ctx.core.device() {
			Some(_) => None,
			None => Some(get_current_library!($ctx)),
		};
		let json_response = $ctx
			.core
			.action(&input, library_id)
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
		// A device-targeted request names a library on the target, whose ids
		// this client does not know; the target resolves its own.
		let library_id = match $ctx.core.device() {
			Some(_) => None,
			None => Some(get_current_library!($ctx)),
		};
		let json_response = $ctx
			.core
			.query(&input, library_id)
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

/// Ask whether and how a library action would run, over the input it takes.
#[macro_export]
macro_rules! execute_validate {
	($ctx:expr, $input:expr) => {{
		let input = $input;
		let library_id = match $ctx.core.device() {
			Some(_) => None,
			None => Some(get_current_library!($ctx)),
		};
		$ctx.core
			.validate(&input, library_id)
			.await
			.map_err(|e| $crate::util::error::improve_core_error(e.to_string()))?
	}};
}

/// Ask what would exist after a library action, over the input it takes.
/// The plan type comes from the binding.
#[macro_export]
macro_rules! execute_preview {
	($ctx:expr, $input:expr) => {{
		let input = $input;
		let library_id = match $ctx.core.device() {
			Some(_) => None,
			None => Some(get_current_library!($ctx)),
		};
		$ctx.core
			.preview(&input, library_id)
			.await
			.map_err(|e| $crate::util::error::improve_core_error(e.to_string()))?
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
