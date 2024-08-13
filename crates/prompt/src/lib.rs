extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Lit, Meta, MetaNameValue, NestedMeta};

#[proc_macro_derive(Prompt, attributes(prompt))]
pub fn prompt_derive(input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as DeriveInput);
	let name = &input.ident;
	let mut instruct = None;

	// Parse the attributes on the struct or enum itself
	for attr in &input.attrs {
		if attr.path.is_ident("prompt") {
			if let Ok(Meta::List(meta_list)) = attr.parse_meta() {
				for nested_meta in meta_list.nested {
					if let NestedMeta::Meta(Meta::NameValue(MetaNameValue { path, lit, .. })) =
						nested_meta
					{
						if path.is_ident("instruct") {
							if let Lit::Str(lit_str) = lit {
								instruct = Some(lit_str.value());
							}
						}
					}
				}
			}
		}
	}

	// Clone the instruct value before moving it
	let instruct_str = instruct.clone().unwrap_or_default();

	// Handle struct and enum differently
	let expanded = match input.data {
		syn::Data::Struct(ref data) => {
			let fields = data.fields.iter().map(|f| {
				let name = &f.ident;
				let mut weight: Option<u16> = None;
				let mut meaning: Option<String> = None;

				// Parse prompt-specific attributes for fields
				for attr in &f.attrs {
					if attr.path.is_ident("prompt") {
						if let Ok(Meta::List(meta_list)) = attr.parse_meta() {
							for nested_meta in meta_list.nested {
								if let NestedMeta::Meta(Meta::NameValue(MetaNameValue {
									path,
									lit,
									..
								})) = nested_meta
								{
									if path.is_ident("weight") {
										if let Lit::Int(lit_int) = lit {
											weight = Some(lit_int.base10_parse::<u16>().unwrap());
										}
									} else if path.is_ident("meaning") {
										if let Lit::Str(lit_str) = lit {
											meaning = Some(lit_str.value());
										}
									}
								}
							}
						}
					}
				}

				let weight_str = if let Some(weight) = weight {
					format!(" (weight: {})", weight)
				} else {
					"".to_string()
				};

				let meaning_str = if let Some(meaning) = meaning {
					format!(" - {}", meaning)
				} else {
					"".to_string()
				};

				quote! {
					prompts.push(format!("  - {}: {:?}{}{}", stringify!(#name), self.#name, #weight_str, #meaning_str));
				}
			});

			quote! {
				impl Prompt for #name {
					fn generate_prompt(&self) -> String {
						let mut prompts = Vec::new();
						prompts.push(format!("\n* Purpose *: {}\n* Attributes *:", #instruct_str));
						#(#fields)*
						prompts.join("\n")
					}
				}
			}
		}
		syn::Data::Enum(ref data) => {
			let variants = data.variants.iter().map(|v| {
				let variant_name = &v.ident;
				let mut variant_instruct = instruct.clone();

				// Handle prompt-specific instructions for each variant
				for attr in &v.attrs {
					if attr.path.is_ident("prompt") {
						if let Ok(Meta::List(meta_list)) = attr.parse_meta() {
							for nested_meta in meta_list.nested {
								if let NestedMeta::Meta(Meta::NameValue(MetaNameValue {
									path,
									lit,
									..
								})) = nested_meta
								{
									if path.is_ident("instruct") {
										if let Lit::Str(lit_str) = lit {
											variant_instruct = Some(lit_str.value());
										}
									}
								}
							}
						}
					}
				}

				let variant_instruct_str = variant_instruct.unwrap_or_else(|| instruct_str.clone());

				quote! {
					#name::#variant_name => format!("  - {}: {}", stringify!(#variant_name), #variant_instruct_str),
				}
			});

			quote! {
				impl Prompt for #name {
					fn generate_prompt(&self) -> String {
						match self {
							#(#variants)*
						}
					}
				}
			}
		}
		_ => unimplemented!(),
	};

	TokenStream::from(expanded)
}
