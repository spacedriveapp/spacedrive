use crate::Prompt;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::HashMap;
use std::sync::Mutex;

pub trait Concept: Clone + 'static + Prompt {
	fn store(&self);
	fn retrieve_all() -> Vec<Self>
	where
		Self: Sized;
	fn concept_name() -> &'static str;
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

pub struct AnyConceptWrapper {
	concept: Box<dyn Any>,
	store: Box<dyn Fn(&dyn Any)>,
	generate_prompt: Box<dyn Fn(&dyn Any) -> String>,
	concept_name: &'static str,
}

impl AnyConceptWrapper {
	pub fn new<T: Concept + 'static>(concept: T) -> Self {
		AnyConceptWrapper {
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
					instance: Box::new(self.clone()),
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
							.filter_map(|entry| entry.instance.downcast_ref::<Self>().cloned())
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
