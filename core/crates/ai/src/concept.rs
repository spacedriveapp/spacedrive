use crate::Prompt;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::HashMap;
use std::sync::Mutex;
// Updated Concept trait with Send + Sync
pub trait Concept: Clone + 'static + Prompt + Send + Sync {
	fn store(&self);
	fn retrieve_all() -> Vec<Self>
	where
		Self: Sized;
	fn concept_name() -> &'static str;
}

// Updated ConceptWrapper struct with Send + Sync
pub struct ConceptWrapper {
	pub concept: Box<dyn Any + Send + Sync>,
	store: Box<dyn Fn(&dyn Any) + Send + Sync>,
	generate_prompt: Box<dyn Fn(&dyn Any) -> String + Send + Sync>,
	concept_name: &'static str,
}

pub struct ConceptEntry {
	pub instance: Box<dyn Any + Send + Sync>,
}

pub static CONCEPT_REGISTRY: Lazy<Mutex<HashMap<&'static str, Vec<ConceptEntry>>>> =
	Lazy::new(|| Mutex::new(HashMap::new()));

pub fn register_concept<T: Concept>() {
	let mut registry = CONCEPT_REGISTRY.lock().unwrap();
	registry.entry(T::concept_name()).or_insert_with(Vec::new);
}

impl ConceptWrapper {
	pub fn new<T: Concept + 'static>(concept: T) -> Self {
		ConceptWrapper {
			concept: Box::new(concept),
			store: Box::new(|c| c.downcast_ref::<T>().unwrap().store()),
			generate_prompt: Box::new(|c| c.downcast_ref::<T>().unwrap().generate_prompt()),
			concept_name: T::concept_name(),
		}
	}

	pub fn store(&self) {
		(self.store)(&*self.concept);
	}

	pub fn generate_prompt(&self) -> String {
		(self.generate_prompt)(&*self.concept)
	}

	pub fn concept_name(&self) -> &'static str {
		self.concept_name
	}
}

#[macro_export]
macro_rules! define_concept {
	($concept_name:ident) => {
		impl Concept for $concept_name {
			fn store(&self) {
				let mut registry = $crate::CONCEPT_REGISTRY.lock().unwrap();
				let entry = $crate::concept::ConceptEntry {
					instance: Box::new($crate::concept::ConceptWrapper::new(self.clone())),
				};
				registry
					.entry(Self::concept_name())
					.or_insert_with(Vec::new)
					.push(entry);
			}

			fn retrieve_all() -> Vec<Self> {
				let registry = $crate::CONCEPT_REGISTRY.lock().unwrap();
				registry
					.get(Self::concept_name())
					.map(|instances| {
						instances
							.iter()
							.filter_map(|entry| {
								entry
									.instance
									.downcast_ref::<$crate::concept::ConceptWrapper>()
									.and_then(|wrapped| {
										wrapped.concept.downcast_ref::<Self>().cloned()
									})
							})
							.collect()
					})
					.unwrap_or_else(Vec::new)
			}

			fn concept_name() -> &'static str {
				stringify!($concept_name)
			}
		}
	};
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct ConceptRequest {
	pub name: String,
	pub filters: Option<HashMap<String, String>>,
}

// list all concepts with their meta extracted from the Prompt derive
#[derive(Serialize, Deserialize, Debug)]
pub struct ConceptMeta {
	pub name: String,
	pub instruct: String,
	// pub meaning: String,
}

impl Prompt for ConceptMeta {
	fn generate_prompt(&self) -> String {
		format!("{}: {}", self.name, self.instruct)
	}
}

pub fn list_concepts() -> Vec<ConceptMeta> {
	let registry = CONCEPT_REGISTRY.lock().unwrap();
	registry
		.iter()
		.map(|(concept_name, entries)| {
			if let Some(first_entry) = entries.first() {
				// Downcast to the actual concept type and call generate_prompt
				let instruct = if let Some(concept_wrapper) =
					first_entry.instance.downcast_ref::<ConceptWrapper>()
				{
					concept_wrapper.generate_prompt()
				} else {
					String::from("No instruct available")
				};

				ConceptMeta {
					name: concept_name.to_string(),
					instruct,
				}
			} else {
				ConceptMeta {
					name: concept_name.to_string(),
					instruct: String::from("No instruct available"),
				}
			}
		})
		.collect()
}
