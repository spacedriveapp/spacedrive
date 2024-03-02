use std::{
	hash::{Hash, Hasher},
	marker::PhantomData,
	sync::Arc,
};

use serde::{ser::SerializeMap, Serialize, Serializer};
use specta::{Any, DataType, NamedType, Type, TypeMap};

/// A type that can be used to return a group of `Reference<T>` and `CacheNode`'s
///
/// You don't need to use this, it's just a shortcut to avoid having to write out the full type every time.
#[derive(Serialize, Type, Debug)]
pub struct NormalisedResults<T: Model + Type> {
	pub items: Vec<Reference<T>>,
	pub nodes: Vec<CacheNode>,
}

/// A type that can be used to return a group of `Reference<T>` and `CacheNode`'s
///
/// You don't need to use this, it's just a shortcut to avoid having to write out the full type every time.
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
/// If you use a `Reference` in a query, you *must* ensure the corresponding `CacheNode` is also in the query.
#[derive(Type, Debug, Clone, Hash, PartialEq, Eq)]
pub struct Reference<T> {
	__type: &'static str,
	__id: String,
	#[specta(rename = "#type")]
	ty: PhantomType<T>,
}

impl<T: Model + Type> Reference<T> {
	pub fn new(key: String) -> Self {
		Self {
			__type: "", // This is just to fake the field for Specta
			__id: key,
			ty: PhantomType(PhantomData),
		}
	}
}

impl<T: Model> Serialize for Reference<T> {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		let mut map = serializer.serialize_map(Some(2))?;
		map.serialize_entry("__type", T::name())?;
		map.serialize_entry("__id", &self.__id)?;
		map.end()
	}
}

/// A node in the cache.
/// This holds the data and is identified by it's type and id.
#[derive(Debug, Clone)]
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

impl PartialEq for CacheNode {
	fn eq(&self, other: &Self) -> bool {
		self.0 == other.0
			&& self.1 == other.1
			&& match (&self.2, &other.2) {
				(Ok(v0), Ok(v1)) => v0 == v1,
				// Compares the values in the Arcs, not the Arc objects themselves.
				(Err(e0), Err(e1)) => {
					(*e0).classify() == (*e1).classify()
						&& (*e0).column() == (*e1).column()
						&& (*e0).line() == (*e1).line()
				}
				_ => false,
			}
	}
}

impl Eq for CacheNode {}

impl Hash for CacheNode {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.0.hash(state);
		self.1.as_str().hash(state);
		self.1.as_str().hash(state);
	}
}

#[derive(Type, Default)]
#[specta(rename = "CacheNode", remote = CacheNode)]
#[allow(unused)]
struct CacheNodeTy {
	__type: String,
	__id: String,
	#[specta(rename = "#node")]
	node: Any,
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
				serde::ser::Error::custom(format!("Failed to serialize node: {}", err))
			})?,
		}
		.serialize(serializer)
	}
}

/// A helper for easily normalizing data.
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

/// Basically `PhantomData`.
///
/// With Specta `PhantomData` is exported as `null`.
/// This will export as `T` but serve the same purpose as `PhantomData` (holding a type without it being instantiated).
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct PhantomType<T>(PhantomData<T>);

/// WARNING: This type is surgically updated within `Reference` in the final typedefs due it being impossible to properly implement.
/// Be careful changing it!

impl<T: Type> Type for PhantomType<T> {
	fn inline(type_map: &mut TypeMap, generics: &[DataType]) -> DataType {
		T::inline(type_map, generics)
	}

	fn reference(type_map: &mut TypeMap, generics: &[DataType]) -> specta::reference::Reference {
		T::reference(type_map, generics)
	}

	fn definition(type_map: &mut TypeMap) -> DataType {
		T::definition(type_map)
	}
}

// This function is cursed.
pub fn patch_typedef(type_map: &mut TypeMap) {
	#[derive(Type)]
	#[specta(rename = "Reference")]
	#[allow(unused)]
	struct ReferenceTy<T> {
		__type: &'static str,
		__id: String,
		#[specta(rename = "#type")]
		ty: T,
	}

	let mut def = <Reference<()> as NamedType>::definition_named_data_type(type_map);
	def.inner = ReferenceTy::<Any>::definition(type_map);
	type_map.insert(<Reference<()> as NamedType>::SID, def)
}
