use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

pub trait Capability: 'static + Send + Sync {
	fn name(&self) -> &'static str;
	fn description(&self) -> &'static str;
	fn execute(&self, args: &[String]) -> Result<String, String>;
}

pub static CAPABILITY_REGISTRY: Lazy<Mutex<HashMap<&'static str, Box<dyn Capability>>>> =
	Lazy::new(|| Mutex::new(HashMap::new()));

pub fn register_capability<T: Capability + 'static>(capability: T) {
	let mut registry = CAPABILITY_REGISTRY.lock().unwrap();
	registry.insert(capability.name(), Box::new(capability));
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CapabilityRequest {
	pub name: String,
	pub args: Vec<String>,
}
