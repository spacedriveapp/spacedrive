extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Fields, Lit, Meta, NestedMeta};

#[proc_macro_derive(Prompt, attributes(prompt))]
pub fn prompt_derive(input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as DeriveInput);
	let name = &input.ident;

	let mut instruct_str = proc_macro2::TokenStream::new();
	let mut show_variants = false;
	let mut show_schema = false; // New flag to control schema inclusion
	let mut variant_instructions = Vec::new();

	// Extract attributes from the struct/enum itself
	for attr in &input.attrs {
		if attr.path.is_ident("prompt") {
			if let Ok(Meta::List(meta_list)) = attr.parse_meta() {
				for nested_meta in meta_list.nested.iter() {
					if let NestedMeta::Meta(Meta::NameValue(meta_name_value)) = nested_meta {
						if meta_name_value.path.is_ident("instruct") {
							if let Lit::Str(lit_str) = &meta_name_value.lit {
								instruct_str = quote! { #lit_str };
							}
						} else if meta_name_value.path.is_ident("show_variants") {
							if let Lit::Bool(lit_bool) = &meta_name_value.lit {
								show_variants = lit_bool.value;
							}
						} else if meta_name_value.path.is_ident("show_schema") {
							// Check for show_schema
							if let Lit::Bool(lit_bool) = &meta_name_value.lit {
								show_schema = lit_bool.value;
							}
						}
					}
				}
			}
		}
	}

	// Handle enum variants
	if let syn::Data::Enum(data_enum) = &input.data {
		for variant in &data_enum.variants {
			if let Some(attr) = variant.attrs.iter().find(|a| a.path.is_ident("prompt")) {
				if let Ok(Meta::List(meta_list)) = attr.parse_meta() {
					for nested_meta in meta_list.nested.iter() {
						if let NestedMeta::Meta(Meta::NameValue(meta_name_value)) = nested_meta {
							if meta_name_value.path.is_ident("instruct") {
								if let Lit::Str(lit_str) = &meta_name_value.lit {
									let variant_name = variant.ident.to_string();
									let value = lit_str.value();
									variant_instructions.push(quote! {
										variant_map.insert(
											#variant_name.to_string(),
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
	let schema = if show_schema {
		quote! {
			let schema_str = serde_json::to_string_pretty(&#name::provide_schema()).unwrap();
			prompt_map.insert("schema".to_string(), serde_json::from_str(&schema_str).unwrap());
		}
	} else {
		quote! {}
	};

	// Combine all instructions into the prompt with pretty printing
	let expanded = quote! {
		impl Prompt for #name {
			fn generate_prompt(&self) -> String {
				let mut prompt_map = serde_json::Map::new();

				prompt_map.insert("instruct".to_string(), serde_json::Value::String(#instruct_str.to_string()));

				#schema  // Insert schema conditionally

				#[allow(unused_mut)]
				let mut variant_map = serde_json::Map::new();
				#(#variant_instructions)*

				if #show_variants && !variant_map.is_empty() {
					prompt_map.insert("variants".to_string(), serde_json::Value::Object(variant_map));
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
