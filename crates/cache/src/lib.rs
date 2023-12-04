use std::{marker::PhantomData, sync::Arc};

use serde::{ser::SerializeMap, Serialize, Serializer};
use specta::{DataType, DefOpts, Type};

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

// // TODO: We could cleanup this file using `DataTypeFrom` but it's `rename` is broken.
// fn build(ty_name_type: DataType, ty_type_type: DataType) -> DataType {
// 	DataType::Object(specta::ObjectType {
// 		generics: vec![],
// 		fields: vec![
// 			specta::ObjectField {
// 				key: "__type",
// 				optional: false,
// 				flatten: false,
// 				ty: ty_name_type,
// 			},
// 			specta::ObjectField {
// 				key: "__id",
// 				optional: false,
// 				flatten: false,
// 				ty: DataType::Primitive(specta::PrimitiveType::String),
// 			},
// 			specta::ObjectField {
// 				key: "#type",
// 				optional: false,
// 				flatten: false,
// 				ty: ty_type_type,
// 			},
// 		],
// 		tag: None,
// 	})
// }

// const SID: specta::r#type::TypeSid = specta::sid!(@with_specta_path; "Reference"; specta);

impl<T: Model + Type> Type for Reference<T> {
	fn inline(opts: DefOpts, generics: &[DataType]) -> DataType {
		todo!()
	}

	// fn inline(opts: DefOpts, generics: &[DataType]) -> Result<DataType, ExportError> {
	// 	// Ok(build(
	// 	// 	DataType::Literal(specta::LiteralType::String(T::name().to_string())),
	// 	// 	T::inline(opts, generics)?,
	// 	// ))

	// 	Ok(DataType::Reference(DataTypeReference {
	// 		name: "Reference",
	// 		sid: SID,
	// 		generics: vec![T::inline(opts, generics)?],
	// 	}))
	// }

	// fn definition_generics() -> Vec<specta::GenericType> {
	// 	vec![GenericType("T")]
	// }

	// fn reference(opts: DefOpts, generics: &[DataType]) -> Result<DataType, ExportError> {
	// 	Ok(build(
	// 		DataType::Literal(specta::LiteralType::String(T::name().to_string())),
	// 		DataType::Any, // T::reference(opts, generics)?,
	// 		               // DataType::Primitive(PrimitiveType::String),
	// 		               // DataType::Generic(GenericType("T")),
	// 	))
	// }

	// fn definition(_opts: DefOpts) -> Result<DataType, ExportError> {
	// 	Ok(build(
	// 		DataType::Primitive(PrimitiveType::String),
	// 		DataType::Generic(GenericType("T")),
	// 	))
	// }

	// fn category_impl(
	// 	_opts: DefOpts,
	// 	_generics: &[DataType],
	// ) -> Result<specta::TypeCategory, ExportError> {
	// 	Ok(TypeCategory::Reference(DataTypeReference {
	// 		name: "Reference",
	// 		sid: SID,
	// 		generics: vec![DataType::Generic(GenericType("T"))],
	// 	}))
	// }
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
	fn inline(opts: DefOpts, generics: &[DataType]) -> DataType {
		todo!()
	}

	// fn inline(_opts: DefOpts, _generics: &[DataType]) -> Result<DataType, ExportError> {
	// 	Ok(DataType::Object(specta::ObjectType {
	// 		generics: vec![],
	// 		fields: vec![
	// 			specta::ObjectField {
	// 				key: "__type",
	// 				optional: false,
	// 				flatten: false,
	// 				ty: DataType::Primitive(specta::PrimitiveType::String),
	// 			},
	// 			specta::ObjectField {
	// 				key: "__id",
	// 				optional: false,
	// 				flatten: false,
	// 				ty: DataType::Primitive(specta::PrimitiveType::String),
	// 			},
	// 			specta::ObjectField {
	// 				key: "#node",
	// 				optional: false,
	// 				flatten: false,
	// 				ty: DataType::Any,
	// 			},
	// 			// We ignore the extra fields because they can't be properly typed.
	// 		],
	// 		tag: None,
	// 	}))
	// }
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
