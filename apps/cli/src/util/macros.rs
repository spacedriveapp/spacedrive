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
			.map_err(|e| CliError::CoreError(e.to_string()))?;
		bincode::deserialize(&bytes).map_err(|e| {
			CliError::SerializationError(format!("Failed to deserialize response: {}", e))
		})?
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
			.map_err(|e| CliError::CoreError(e.to_string()))?
	}};
}

/// Print output in the configured format (human or JSON)
#[macro_export]
macro_rules! print_output {
	($ctx:expr, $output:expr, $human:expr) => {{
		match $ctx.format {
			crate::context::OutputFormat::Human => {
				$human($output);
			}
			crate::context::OutputFormat::Json => {
				crate::util::output::print_json(&$output);
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
			.ok_or(CliError::NoActiveLibrary)?
	}};
}
