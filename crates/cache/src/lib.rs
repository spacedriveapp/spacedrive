use std::{marker::PhantomData, sync::Arc};

use serde::{ser::SerializeMap, Serialize, Serializer};
use specta::Type;

/// A type that can be used to return a group of `Reference<T>` and `CacheNode`'s
///
/// You don't need to use this, it's just a shortcut to avoid having to write out the full type everytime.
#[derive(Serialize, Type, Debug)]
pub struct NormalisedResults<T: Model + Type> {
	pub items: Vec<Reference<T>>,
	pub nodes: Vec<CacheNode>,
}

/// A type that can be used to return a group of `Reference<T>` and `CacheNode`'s
///
/// You don't need to use this, it's just a shortcut to avoid having to write out the full type everytime.
#[derive(Serialize, Type, Debug)]
pub struct NormalisedResult<T: Model + Type> {
	pub item: Reference<T>,
	pub nodes: Vec<CacheNode>,
}

impl<T: Model + Serialize + Type> NormalisedResult<T> {
	pub fn from(item: T, id_fn: impl Fn(&T) -> String) -> Self {
		let id = id_fn(&item);
		Self {
			item: Reference::new(id.clone()),
			nodes: vec![CacheNode::new(id, item)],
		}
	}
}

/// A type which can be stored in the cache.
pub trait Model {
	/// Must return a unique identifier for this model within the cache.
	fn name() -> &'static str;
}

/// A reference to a `CacheNode`.
///
/// This does not contain the actual data, but instead a reference to it.
/// This allows the CacheNode's to be switched out and the query recomputed without any backend communication.
///
/// If you use a `Reference` in a query, you *must* ensure the `CacheNode` in also in the query.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Reference<T>(String, PhantomData<T>);

impl<T: Model + Type> Reference<T> {
	pub fn new(key: String) -> Self {
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
					ty: specta::DataType::Primitive(specta::PrimitiveType::String),
				},
				specta::ObjectField {
					key: "#type",
					optional: false,
					flatten: false,
					ty: T::reference(opts, generics)?,
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

/// A node in the cache.
/// This holds the data and is identified by it's type and id.
#[derive(Debug, Clone)] // TODO: `Hash, PartialEq, Eq`
pub struct CacheNode(
	&'static str,
	serde_json::Value,
	Result<serde_json::Value, Arc<serde_json::Error>>,
);

impl CacheNode {
	pub fn new<T: Model + Serialize + Type>(key: String, value: T) -> Self {
		Self(
			T::name(),
			key.into(),
			serde_json::to_value(value).map_err(Arc::new),
		)
	}
}

impl Type for CacheNode {
	fn inline(
		_opts: specta::DefOpts,
		_generics: &[specta::DataType],
	) -> Result<specta::DataType, specta::ExportError> {
		Ok(specta::DataType::Object(specta::ObjectType {
			generics: vec![],
			fields: vec![
				specta::ObjectField {
					key: "__type",
					optional: false,
					flatten: false,
					ty: specta::DataType::Primitive(specta::PrimitiveType::String),
				},
				specta::ObjectField {
					key: "__id",
					optional: false,
					flatten: false,
					ty: specta::DataType::Primitive(specta::PrimitiveType::String),
				},
				specta::ObjectField {
					key: "#node",
					optional: false,
					flatten: false,
					ty: specta::DataType::Any,
				},
				// We ignore the extra fields because they can't be properly typed.
			],
			tag: None,
		}))
	}
}

#[derive(Serialize)]
struct NodeSerdeRepr<'a> {
	__type: &'static str,
	__id: &'a serde_json::Value,
	#[serde(flatten)]
	v: &'a serde_json::Value,
}

impl Serialize for CacheNode {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		NodeSerdeRepr {
			__type: self.0,
			__id: &self.1,
			v: self.2.as_ref().map_err(|err| {
				serde::ser::Error::custom(format!("Failed to serialise node: {}", err))
			})?,
		}
		.serialize(serializer)
	}
}

/// A helper for easily normalising data.
pub trait Normalise {
	type Item: Model + Type;

	fn normalise(
		self,
		id_fn: impl Fn(&Self::Item) -> String,
	) -> (Vec<CacheNode>, Vec<Reference<Self::Item>>);
}

impl<T: Model + Serialize + Type> Normalise for Vec<T> {
	type Item = T;

	fn normalise(
		self,
		id_fn: impl Fn(&Self::Item) -> String,
	) -> (Vec<CacheNode>, Vec<Reference<Self::Item>>) {
		let mut nodes = Vec::with_capacity(self.len());
		let mut references = Vec::with_capacity(self.len());

		for item in self.into_iter() {
			let id = id_fn(&item);
			nodes.push(CacheNode::new(id.clone(), item));
			references.push(Reference::new(id));
		}

		(nodes, references)
	}
}
