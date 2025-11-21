//! JSON export.

use anyhow::Result;
use serde::Serialize;

use crate::types::{LogGroup, Template};

#[derive(Serialize)]
struct JsonExport<'a> {
	templates: &'a [Template],
	groups: &'a [LogGroup],
}

/// Export analysis to JSON.
pub fn export_json(templates: &[Template], groups: &[LogGroup]) -> Result<String> {
	let export = JsonExport { templates, groups };
	Ok(serde_json::to_string_pretty(&export)?)
}




