extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Lit, Meta, NestedMeta};

#[proc_macro_derive(Prompt, attributes(prompt))]
pub fn prompt_derive(input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as DeriveInput);
	let name = &input.ident;

	let mut instruct_str = proc_macro2::TokenStream::new();
	let mut field_instructions = Vec::new();

	// Extract the `instruct` attribute value from the struct/enum itself
	for attr in &input.attrs {
		if attr.path.is_ident("prompt") {
			if let Ok(Meta::List(meta_list)) = attr.parse_meta() {
				for nested_meta in meta_list.nested.iter() {
					if let NestedMeta::Meta(Meta::NameValue(meta_name_value)) = nested_meta {
						if meta_name_value.path.is_ident("instruct") {
							if let Lit::Str(lit_str) = &meta_name_value.lit {
								let value = lit_str.value();
								instruct_str = quote! {
									#value
								};
							}
						}
					}
				}
			}
		}
	}

	// Handle struct fields
	if let syn::Data::Struct(data_struct) = &input.data {
		for field in &data_struct.fields {
			if let Some(attr) = field.attrs.iter().find(|a| a.path.is_ident("prompt")) {
				if let Ok(Meta::List(meta_list)) = attr.parse_meta() {
					for nested_meta in meta_list.nested.iter() {
						if let NestedMeta::Meta(Meta::NameValue(meta_name_value)) = nested_meta {
							if meta_name_value.path.is_ident("instruct") {
								if let Lit::Str(lit_str) = &meta_name_value.lit {
									let field_name = field.ident.as_ref().unwrap().to_string();
									let value = lit_str.value();
									field_instructions.push(quote! {
										field_map.insert(
											#field_name.to_string(),
											serde_json::Value::String(#value.to_string())
										);
									});
								}
							}
						}
					}
				}
			}
		}
	}

	// Generate the schema using the SchemaProvider trait
	let schema = quote! {
		#name::provide_schema()
	};

	// Combine all instructions into the prompt with pretty printing
	let expanded = quote! {
		impl Prompt for #name {
			fn generate_prompt(&self) -> String {
				let schema_str = serde_json::to_string_pretty(&#schema).unwrap();
				let mut prompt_map = serde_json::Map::new();

				prompt_map.insert("instruct".to_string(), serde_json::Value::String(#instruct_str.to_string()));
				prompt_map.insert("schema".to_string(), serde_json::from_str(&schema_str).unwrap());

				let mut field_map = serde_json::Map::new();
				#(#field_instructions)*

				if !field_map.is_empty() {
					prompt_map.insert("fields".to_string(), serde_json::Value::Object(field_map));
				}

				Self::pretty_print_json(&prompt_map)
			}
		}

		impl #name {
			fn pretty_print_json(value: &serde_json::Map<String, serde_json::Value>) -> String {
				fn print_value(value: &serde_json::Value, indent: usize) -> String {
					match value {
						serde_json::Value::Object(map) => {
							let contents: Vec<String> = map
								.iter()
								.map(|(k, v)| format!("{:indent$}\"{}\": {}", "", k, print_value(v, indent + 2), indent = indent + 2))
								.collect();
							format!("{{\n{}\n{:indent$}}}", contents.join(",\n"), "", indent = indent)
						}
						serde_json::Value::Array(arr) => {
							let contents: Vec<String> = arr
								.iter()
								.map(|v| format!("{:indent$}{}", "", print_value(v, indent + 2), indent = indent + 2))
								.collect();
							format!("[\n{}\n{:indent$}]", contents.join(",\n"), "", indent = indent)
						}
						serde_json::Value::String(s) => format!("\"{}\"", s.replace('"', "\\\"")),
						_ => value.to_string(),
					}
				}

				print_value(&serde_json::Value::Object(value.clone()), 0)
			}
		}
	};

	TokenStream::from(expanded)
}
