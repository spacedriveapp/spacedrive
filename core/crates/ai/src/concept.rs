use crate::Prompt;
use once_cell::sync::Lazy;
use schemars::schema::Schema;
use std::any::TypeId;
use std::collections::HashMap;
use std::sync::Mutex;

pub trait SchemaProvider {
	fn provide_schema() -> Schema;
}

pub trait Concept: Clone + 'static + Prompt + Send + Sync + SchemaProvider {
	fn concept_name() -> &'static str
	where
		Self: Sized;
}

pub static CONCEPT_META_CACHE: Lazy<Mutex<HashMap<TypeId, ConceptMeta>>> =
	Lazy::new(|| Mutex::new(HashMap::new()));

#[macro_export]
macro_rules! define_concept {
	($concept_name:ident) => {
		impl Concept for $concept_name {
			fn concept_name() -> &'static str {
				stringify!($concept_name)
			}
		}

		// Implementing SchemaProvider for the concept
		impl SchemaProvider for $concept_name {
			fn provide_schema() -> schemars::schema::Schema {
				schemars::schema::Schema::Object(schema_for!($concept_name).schema)
			}
		}

		paste::paste! {
			fn [<register_concept_meta_ $concept_name:snake>]() {
				let meta = crate::concept::ConceptMeta {
					name: stringify!($concept_name).to_string(),
					instruct: <$concept_name as crate::Prompt>::generate_prompt(&Default::default()), // This assumes Default is implemented.
				};
				let type_id = std::any::TypeId::of::<$concept_name>();
				let mut cache = crate::concept::CONCEPT_META_CACHE.lock().unwrap();
				cache.insert(type_id, meta);
			}

			#[ctor::ctor]
			fn [<init_ $concept_name:snake>]() {
				[<register_concept_meta_ $concept_name:snake>]();
			}
		}
	};
}

#[derive(Debug, Clone)]
pub struct ConceptMeta {
	pub name: String,
	pub instruct: String,
}

impl Prompt for ConceptMeta {
	fn generate_prompt(&self) -> String {
		format!("[{}]: {}", self.name, self.instruct)
	}
}

pub fn get_concept_meta<T: Concept + Default>() -> ConceptMeta {
	let type_id = TypeId::of::<T>();
	let mut cache = CONCEPT_META_CACHE.lock().unwrap();

	cache
		.entry(type_id)
		.or_insert_with(|| ConceptMeta {
			name: T::concept_name().to_string(),
			instruct: T::default().generate_prompt(),
		})
		.clone()
}

pub fn list_concepts() -> Vec<ConceptMeta> {
	let cache = CONCEPT_META_CACHE.lock().unwrap();
	cache.values().cloned().collect()
}
