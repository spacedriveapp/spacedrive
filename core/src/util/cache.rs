use std::marker::PhantomData;

use serde::{ser::SerializeMap, Serialize, Serializer};
use specta::Type;

pub trait Model {
	/// Must return a unique identifier for this model within the cache.
	fn name() -> &'static str;
}

pub struct Reference<T>(serde_json::Value, PhantomData<T>);

impl<T: Model + Type> Reference<T> {
	pub fn new(key: impl Into<serde_json::Value>) -> Self {
		Self(key.into(), PhantomData)
	}
}

impl<T: Model + Type> Type for Reference<T> {
	fn inline(
		opts: specta::DefOpts,
		generics: &[specta::DataType],
	) -> Result<specta::DataType, specta::ExportError> {
		Ok(specta::DataType::Object(specta::ObjectType {
			generics: vec![],
			fields: vec![
				specta::ObjectField {
					key: "__type",
					optional: false,
					flatten: false,
					ty: specta::DataType::Literal(specta::LiteralType::String(
						T::name().to_string(),
					)),
				},
				specta::ObjectField {
					key: "__id",
					optional: false,
					flatten: false,
					ty: specta::DataType::Any,
				},
				specta::ObjectField {
					key: "#type",
					optional: false,
					flatten: false,
					ty: T::inline(opts, generics)?,
				},
			],
			tag: None,
		}))
	}
}

impl<T: Model> Serialize for Reference<T> {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		let mut map = serializer.serialize_map(Some(2))?;
		map.serialize_entry("__type", T::name())?;
		map.serialize_entry("__id", &self.0)?;
		map.end()
	}
}
