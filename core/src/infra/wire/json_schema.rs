//! # JSON Schema from Specta types
//!
//! Every registered operation's input derives `specta::Type`, which is what the
//! TypeScript and Swift clients are generated from. Transports that describe
//! their inputs as JSON Schema, such as the MCP server, need the same type
//! walked into a schema document, so the schema an agent sees is the shape
//! serde actually accepts and never drifts from the Rust type.
//!
//! The output is self-contained: named types are inlined rather than collected
//! under `$defs`, because tool schemas are read one at a time. A type that
//! refers to itself gets an unconstrained `{}` at the recursion point.
//!
//! ## Example
//! ```rust,ignore
//! use specta::{Type, TypeCollection};
//! use sd_core::infra::wire::json_schema::json_schema;
//!
//! let mut types = TypeCollection::default();
//! let ty = <MyInput as Type>::definition(&mut types);
//! let schema = json_schema(&ty, &types);
//! ```

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};
use specta::{
	datatype::{DataType, Enum, EnumRepr, Field, Fields, Generic, Literal, Primitive},
	SpectaID, TypeCollection,
};

/// Convert a Specta data type into a self-contained JSON Schema.
pub fn json_schema(ty: &DataType, types: &TypeCollection) -> Value {
	let mut walker = Walker {
		types,
		generics: BTreeMap::new(),
		visiting: Vec::new(),
	};
	walker.convert(ty)
}

/// Whether a schema accepts a JSON object, which is what a tool's top-level
/// input has to be.
pub fn is_object_schema(schema: &Value) -> bool {
	schema.get("type").and_then(Value::as_str) == Some("object")
}

struct Walker<'a> {
	types: &'a TypeCollection,
	generics: BTreeMap<Generic, DataType>,
	visiting: Vec<SpectaID>,
}

impl Walker<'_> {
	fn convert(&mut self, ty: &DataType) -> Value {
		match ty {
			DataType::Primitive(p) => primitive(p),
			DataType::Literal(l) => literal(l),
			DataType::List(list) => {
				let mut schema = json!({ "type": "array", "items": self.convert(list.ty()) });
				if let Some(len) = list.length() {
					schema["minItems"] = json!(len);
					schema["maxItems"] = json!(len);
				}
				if list.unique() {
					schema["uniqueItems"] = json!(true);
				}
				schema
			}
			DataType::Map(map) => {
				json!({ "type": "object", "additionalProperties": self.convert(map.value_ty()) })
			}
			DataType::Nullable(inner) => {
				let inner = self.convert(inner);
				json!({ "anyOf": [inner, { "type": "null" }] })
			}
			DataType::Struct(s) => self.fields(s.fields(), s.tag().map(|t| t.to_string()), None),
			DataType::Enum(e) => self.enumeration(e),
			DataType::Tuple(t) => {
				let elements = t.elements();
				if elements.is_empty() {
					return json!({ "type": "null" });
				}
				let items: Vec<Value> = elements.iter().map(|e| self.convert(e)).collect();
				json!({
					"type": "array",
					"prefixItems": items,
					"minItems": elements.len(),
					"maxItems": elements.len(),
				})
			}
			DataType::Reference(reference) => {
				let Some(named) = self.types.get(reference.sid()) else {
					return json!({});
				};
				if self.visiting.contains(&reference.sid()) {
					return json!({ "description": format!("{} (recursive)", named.name()) });
				}
				// The reference's generic arguments are spelled in the outer
				// scope, so resolve them there before entering the named type.
				let generics: BTreeMap<Generic, DataType> = reference
					.generics()
					.iter()
					.map(|(g, dt)| (g.clone(), self.resolve_generics(dt)))
					.collect();
				let outer = std::mem::replace(&mut self.generics, generics);
				self.visiting.push(reference.sid());
				let mut schema = self.convert(named.ty());
				self.visiting.pop();
				self.generics = outer;
				describe(&mut schema, named.docs());
				schema
			}
			DataType::Generic(g) => match self.generics.get(g).cloned() {
				Some(dt) => self.convert(&dt),
				None => json!({}),
			},
		}
	}

	fn resolve_generics(&self, ty: &DataType) -> DataType {
		match ty {
			DataType::Generic(g) => self.generics.get(g).cloned().unwrap_or_else(|| ty.clone()),
			DataType::Nullable(inner) => DataType::Nullable(Box::new(self.resolve_generics(inner))),
			DataType::List(list) => {
				let mut resolved = list.clone();
				resolved.set_ty(self.resolve_generics(list.ty()));
				DataType::List(resolved)
			}
			DataType::Map(map) => {
				let mut resolved = map.clone();
				resolved.set_key_ty(self.resolve_generics(map.key_ty()));
				resolved.set_value_ty(self.resolve_generics(map.value_ty()));
				DataType::Map(resolved)
			}
			DataType::Reference(reference) => {
				let mut resolved = reference.clone();
				for (_, dt) in resolved.generics_mut().iter_mut() {
					*dt = self.resolve_generics(dt);
				}
				DataType::Reference(resolved)
			}
			other => other.clone(),
		}
	}

	/// A struct or variant body. `tag` carries an internally tagged enum's
	/// discriminator into the variant's object as a constant property.
	fn fields(&mut self, fields: &Fields, tag: Option<String>, variant: Option<&str>) -> Value {
		match fields {
			Fields::Unit => match (tag, variant) {
				(Some(tag), Some(name)) => json!({
					"type": "object",
					"properties": { tag.clone(): { "const": name } },
					"required": [tag],
				}),
				_ => json!({ "type": "null" }),
			},
			Fields::Unnamed(unnamed) => {
				let present: Vec<&Field> = unnamed
					.fields()
					.iter()
					.filter(|f| f.ty().is_some())
					.collect();
				match present.as_slice() {
					[] => json!({ "type": "null" }),
					[single] => {
						let mut schema = self.field_schema(single);
						describe(&mut schema, single.docs());
						schema
					}
					many => {
						let items: Vec<Value> = many.iter().map(|f| self.field_schema(f)).collect();
						json!({
							"type": "array",
							"prefixItems": items,
							"minItems": many.len(),
							"maxItems": many.len(),
						})
					}
				}
			}
			Fields::Named(named) => {
				let mut properties = Map::new();
				let mut required = Vec::new();
				let mut flattened = Vec::new();
				if let (Some(tag), Some(name)) = (tag.as_deref(), variant) {
					properties.insert(tag.to_string(), json!({ "const": name }));
					required.push(json!(tag));
				}
				for (name, field) in named.fields() {
					let Some(ty) = field.ty() else {
						continue;
					};
					if field.flatten() {
						flattened.push(self.convert(ty));
						continue;
					}
					let mut schema = self.convert(ty);
					describe(&mut schema, field.docs());
					properties.insert(name.to_string(), schema);
					// serde fills a missing Option field with None, so only a
					// field that is neither optional nor nullable is required.
					if !field.optional() && !matches!(ty, DataType::Nullable(_)) {
						required.push(json!(name));
					}
				}
				let mut schema = json!({ "type": "object", "properties": properties });
				if !required.is_empty() {
					schema["required"] = Value::Array(required);
				}
				if flattened.is_empty() {
					return schema;
				}
				flattened.insert(0, schema);
				json!({ "allOf": flattened })
			}
		}
	}

	fn field_schema(&mut self, field: &Field) -> Value {
		match field.ty() {
			Some(ty) => self.convert(ty),
			None => json!({}),
		}
	}

	fn enumeration(&mut self, e: &Enum) -> Value {
		let variants: Vec<(&str, &specta::datatype::EnumVariant)> = e
			.variants()
			.iter()
			.filter(|(_, v)| !v.skip())
			.map(|(name, v)| (name.as_ref(), v))
			.collect();
		let repr = e.repr().cloned().unwrap_or(EnumRepr::External);

		if variants
			.iter()
			.all(|(_, v)| matches!(v.fields(), Fields::Unit))
			&& !matches!(repr, EnumRepr::Internal { .. } | EnumRepr::Adjacent { .. })
		{
			let names: Vec<Value> = variants.iter().map(|(n, _)| json!(n)).collect();
			return json!({ "type": "string", "enum": names });
		}

		let options: Vec<Value> = variants
			.iter()
			.map(|(name, variant)| {
				let mut schema = match &repr {
					EnumRepr::Untagged | EnumRepr::String { .. } => {
						self.fields(variant.fields(), None, None)
					}
					EnumRepr::External => match variant.fields() {
						Fields::Unit => json!({ "const": name }),
						fields => {
							let inner = self.fields(fields, None, None);
							json!({
								"type": "object",
								"properties": { *name: inner },
								"required": [name],
								"additionalProperties": false,
							})
						}
					},
					EnumRepr::Internal { tag } => {
						self.fields(variant.fields(), Some(tag.to_string()), Some(name))
					}
					EnumRepr::Adjacent { tag, content } => {
						let mut properties = Map::new();
						properties.insert(tag.to_string(), json!({ "const": name }));
						let mut required = vec![json!(tag)];
						if !matches!(variant.fields(), Fields::Unit) {
							properties.insert(
								content.to_string(),
								self.fields(variant.fields(), None, None),
							);
							required.push(json!(content));
						}
						json!({ "type": "object", "properties": properties, "required": required })
					}
				};
				describe(&mut schema, variant.docs());
				schema
			})
			.collect();

		match options.len() {
			0 => json!({ "not": {} }),
			1 => options.into_iter().next().expect("one option"),
			_ => json!({ "anyOf": options }),
		}
	}
}

fn primitive(p: &Primitive) -> Value {
	match p {
		Primitive::i8
		| Primitive::i16
		| Primitive::i32
		| Primitive::i64
		| Primitive::i128
		| Primitive::isize => json!({ "type": "integer" }),
		Primitive::u8
		| Primitive::u16
		| Primitive::u32
		| Primitive::u64
		| Primitive::u128
		| Primitive::usize => json!({ "type": "integer", "minimum": 0 }),
		Primitive::f16 | Primitive::f32 | Primitive::f64 => json!({ "type": "number" }),
		Primitive::bool => json!({ "type": "boolean" }),
		Primitive::char => json!({ "type": "string", "minLength": 1, "maxLength": 1 }),
		Primitive::String => json!({ "type": "string" }),
	}
}

fn literal(l: &Literal) -> Value {
	match l {
		Literal::i8(v) => json!({ "const": v }),
		Literal::i16(v) => json!({ "const": v }),
		Literal::i32(v) => json!({ "const": v }),
		Literal::u8(v) => json!({ "const": v }),
		Literal::u16(v) => json!({ "const": v }),
		Literal::u32(v) => json!({ "const": v }),
		Literal::f32(v) => json!({ "const": v }),
		Literal::f64(v) => json!({ "const": v }),
		Literal::bool(v) => json!({ "const": v }),
		Literal::String(v) => json!({ "const": v }),
		Literal::char(v) => json!({ "const": v }),
		Literal::None => json!({ "type": "null" }),
		_ => json!({}),
	}
}

/// Attach doc comments as the schema's description, keeping one a nested
/// schema already carries.
fn describe(schema: &mut Value, docs: &str) {
	// Doc comment lines arrive with their leading space.
	let docs = docs
		.lines()
		.map(str::trim)
		.collect::<Vec<_>>()
		.join("\n")
		.trim()
		.to_string();
	if docs.is_empty() {
		return;
	}
	let Value::Object(map) = schema else {
		return;
	};
	map.entry("description")
		.or_insert_with(|| Value::String(docs));
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde::{Deserialize, Serialize};
	use specta::Type;

	#[derive(Serialize, Deserialize, Type)]
	#[serde(rename_all = "snake_case")]
	enum Mode {
		Missing,
		Stale,
	}

	#[derive(Serialize, Deserialize, Type)]
	#[serde(tag = "kind", rename_all = "snake_case")]
	enum Target {
		Path { path: String },
		Everything,
	}

	/// The input to a pretend op.
	#[derive(Serialize, Deserialize, Type)]
	struct Input {
		/// Where to look.
		path: String,
		limit: Option<u32>,
		#[serde(default)]
		recursive: bool,
		mode: Mode,
		target: Target,
		ids: Vec<uuid::Uuid>,
	}

	fn schema_of<T: Type>() -> Value {
		let mut types = TypeCollection::default();
		let ty = T::definition(&mut types);
		json_schema(&ty, &types)
	}

	#[test]
	fn struct_fields_map_to_properties_and_required() {
		let schema = schema_of::<Input>();
		assert!(is_object_schema(&schema));
		assert_eq!(schema["description"], "The input to a pretend op.");
		assert_eq!(schema["properties"]["path"]["type"], "string");
		assert_eq!(
			schema["properties"]["path"]["description"],
			"Where to look."
		);
		assert_eq!(
			schema["properties"]["limit"],
			json!({ "anyOf": [{ "type": "integer", "minimum": 0 }, { "type": "null" }] })
		);
		assert_eq!(schema["properties"]["ids"]["type"], "array");
		let required: Vec<&str> = schema["required"]
			.as_array()
			.unwrap()
			.iter()
			.map(|v| v.as_str().unwrap())
			.collect();
		assert_eq!(required, ["path", "mode", "target", "ids"]);
	}

	#[test]
	fn unit_enums_become_string_enums() {
		let schema = schema_of::<Mode>();
		assert_eq!(
			schema,
			json!({ "type": "string", "enum": ["missing", "stale"] })
		);
	}

	#[test]
	fn internally_tagged_enums_carry_the_tag() {
		let schema = schema_of::<Target>();
		let options = schema["anyOf"].as_array().unwrap();
		assert_eq!(options[0]["properties"]["kind"], json!({ "const": "path" }));
		assert_eq!(options[0]["properties"]["path"]["type"], "string");
		assert_eq!(
			options[1]["properties"]["kind"],
			json!({ "const": "everything" })
		);
	}

	#[test]
	fn unit_input_is_null() {
		assert_eq!(schema_of::<()>(), json!({ "type": "null" }));
	}
}
