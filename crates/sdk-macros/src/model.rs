//! Model macro implementation - generates ExtensionModel trait impl
//!
//! The struct's fields become the model's facet columns in the extension's
//! store. Column types follow the Rust types: strings and uuids are `string`,
//! integers `integer`, floats `float`, bools `boolean`, `DateTime` is
//! `datetime`, `Option<T>` is `T`, and anything else is stored as `json`.
//!
//! The field attributes `#[sidecar]`, `#[sync]`, `#[computed]`,
//! `#[vectorized]`, `#[entry]`, `#[metadata]`, `#[custom_field]` and
//! `#[user_metadata]` are accepted and stripped; nothing acts on them yet, so
//! every field is a column.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Attribute, Data, DeriveInput, Fields, PathArguments, Type};

pub fn model_impl(_args: TokenStream, input: TokenStream) -> TokenStream {
	let mut input = parse_macro_input!(input as DeriveInput);

	// Find the id/uuid field before modifying
	let uuid_field = find_uuid_field(&input);
	let name = input.ident.clone();
	let definition = definition_json(&input);

	// Strip known field attributes (they'll be processed later when macros are enhanced)
	strip_field_attributes(&mut input);

	let expanded = quote! {
		#input

		impl ::spacedrive_sdk::models::ExtensionModel for #name {
			const MODEL_TYPE: &'static str = stringify!(#name);
			const DEFINITION: &'static str = #definition;

			fn uuid(&self) -> ::spacedrive_sdk::types::Uuid {
				self.#uuid_field
			}

			fn search_text(&self) -> String {
				String::new()
			}
		}
	};

	TokenStream::from(expanded)
}

/// The model definition the host registers: `{"name":..,"fields":{..}}`,
/// with fields in declaration order.
fn definition_json(input: &DeriveInput) -> String {
	let mut fields = Vec::new();
	if let Data::Struct(data_struct) = &input.data {
		if let Fields::Named(named) = &data_struct.fields {
			for field in &named.named {
				if let Some(ident) = &field.ident {
					fields.push(format!("\"{}\":\"{}\"", ident, field_type_name(&field.ty)));
				}
			}
		}
	}
	format!(
		"{{\"name\":\"{}\",\"fields\":{{{}}}}}",
		input.ident,
		fields.join(",")
	)
}

fn field_type_name(ty: &Type) -> &'static str {
	let Type::Path(path) = ty else {
		return "json";
	};
	let Some(last) = path.path.segments.last() else {
		return "json";
	};
	let ident = last.ident.to_string();
	if ident == "Option" {
		if let PathArguments::AngleBracketed(generics) = &last.arguments {
			if let Some(syn::GenericArgument::Type(inner)) = generics.args.first() {
				return field_type_name(inner);
			}
		}
		return "json";
	}
	match ident.as_str() {
		"String" | "str" | "Uuid" => "string",
		"i8" | "i16" | "i32" | "i64" | "isize" | "u8" | "u16" | "u32" | "u64" | "usize" => {
			"integer"
		}
		"f32" | "f64" => "float",
		"bool" => "boolean",
		"DateTime" | "NaiveDateTime" => "datetime",
		_ => "json",
	}
}

fn find_uuid_field(input: &DeriveInput) -> syn::Ident {
	if let Data::Struct(data_struct) = &input.data {
		if let Fields::Named(fields) = &data_struct.fields {
			for field in &fields.named {
				if let Some(ident) = &field.ident {
					if ident == "id" || ident == "uuid" {
						return ident.clone();
					}
				}
			}
		}
	}

	syn::Ident::new("id", proc_macro2::Span::call_site())
}

fn strip_field_attributes(input: &mut DeriveInput) {
	if let Data::Struct(ref mut data_struct) = input.data {
		if let Fields::Named(ref mut fields) = data_struct.fields {
			for field in &mut fields.named {
				field.attrs.retain(|attr| !is_model_field_attribute(attr));
			}
		}
	}
}

fn is_model_field_attribute(attr: &Attribute) -> bool {
	let path = &attr.path();

	if let Some(ident) = path.get_ident() {
		let name = ident.to_string();
		matches!(
			name.as_str(),
			"entry"
				| "sidecar" | "metadata"
				| "custom_field"
				| "user_metadata"
				| "computed" | "blob_data"
				| "vectorized"
				| "sync"
		)
	} else {
		false
	}
}
