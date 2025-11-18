//! Variable type inference.

use uuid::Uuid;

use crate::types::VariableType;

/// Infer variable type from observed values.
pub fn infer_variable_type(values: &[&str]) -> VariableType {
	if values.is_empty() {
		return VariableType::String;
	}

	// Check for UUID pattern
	if values.iter().all(|v| Uuid::parse_str(v).is_ok()) {
		return VariableType::Uuid;
	}

	// Check for partial UUID (like device prefix: :1817e146)
	if values.iter().all(|v| {
		v.starts_with(':') && v.len() == 9 && v[1..].chars().all(|c| c.is_ascii_hexdigit())
	}) {
		return VariableType::Uuid;
	}

	// Check for HLC pattern: HLC(timestamp,counter,:device)
	if values
		.iter()
		.all(|v| v.starts_with("HLC(") && v.ends_with(')'))
	{
		return VariableType::HLC;
	}

	// Check for number
	if values
		.iter()
		.all(|v| v.parse::<i64>().is_ok() || v.parse::<f64>().is_ok())
	{
		return VariableType::Number;
	}

	// Check for timestamp (ISO 8601 or similar)
	if values.iter().all(|v| {
		chrono::DateTime::parse_from_rfc3339(v).is_ok() || v.contains("T") && v.contains(':')
	}) {
		return VariableType::Timestamp;
	}

	// Check for file path
	if values.iter().all(|v| v.contains('/') || v.contains('\\')) {
		return VariableType::Path;
	}

	// Check for duration (e.g., "5ms", "2.3s")
	if values
		.iter()
		.all(|v| v.ends_with("ms") || v.ends_with('s') || v.ends_with("us") || v.ends_with("ns"))
	{
		return VariableType::Duration;
	}

	// Default to string
	VariableType::String
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_infer_uuid() {
		let values = vec!["1817e146-53bd-4da2-8ac9-ce430b2e3d15"];
		assert_eq!(infer_variable_type(&values), VariableType::Uuid);
	}

	#[test]
	fn test_infer_uuid_prefix() {
		let values = vec![":1817e146", ":8ef7a321"];
		assert_eq!(infer_variable_type(&values), VariableType::Uuid);
	}

	#[test]
	fn test_infer_number() {
		let values = vec!["123", "456", "789"];
		assert_eq!(infer_variable_type(&values), VariableType::Number);
	}

	#[test]
	fn test_infer_hlc() {
		let values = vec!["HLC(1763277539319,1,:1817e146)"];
		assert_eq!(infer_variable_type(&values), VariableType::HLC);
	}

	#[test]
	fn test_infer_duration() {
		let values = vec!["5ms", "100ms", "2s"];
		assert_eq!(infer_variable_type(&values), VariableType::Duration);
	}

	#[test]
	fn test_infer_string() {
		let values = vec!["hello", "world", "test"];
		assert_eq!(infer_variable_type(&values), VariableType::String);
	}
}


