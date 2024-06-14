use serde::Serialize;

#[allow(clippy::module_name_repetitions)]
#[derive(Serialize)]
pub struct BackendDependency {
	pub title: String,
	pub description: Option<String>,
	pub url: Option<String>,
	pub version: String,
	pub authors: Vec<String>,
	pub license: Option<String>,
}
