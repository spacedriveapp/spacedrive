use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Dependency {
	pub project: Project,
	pub licenses: Vec<License>,
}

#[derive(Serialize, Deserialize)]
pub struct Project {
	// pub locator: Option<String>,
	pub title: String,
	pub description: Option<String>,
	pub url: Option<String>,
	pub authors: Vec<Option<String>>,
}

#[derive(Serialize, Deserialize)]
pub struct License {
	pub text: Option<String>,
	pub license_id: Option<String>,
	pub copyright: Option<String>,
	// pub license_group_id: i64,
	// pub ignored: bool, // always false from my testing
	// pub revision_id: Option<String>,
}

#[allow(clippy::module_name_repetitions)]
#[derive(Serialize)]
pub struct FrontendDependency {
	pub title: String,
	pub description: Option<String>,
	pub url: Option<String>,
	pub authors: Vec<Option<String>>,
	pub license: Vec<License>,
}
