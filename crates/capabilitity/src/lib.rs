use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Fields, Lit, Meta, NestedMeta};

#[proc_macro_derive(Capability, attributes(capability))]
pub fn capability_derive(input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as DeriveInput);
	let name = &input.ident;

	let mut cap_name = String::new();
	let mut description = String::new();

	// Parse the attributes
	for attr in &input.attrs {
		if attr.path.is_ident("capability") {
			if let Ok(Meta::List(meta_list)) = attr.parse_meta() {
				for nested in meta_list.nested.iter() {
					if let NestedMeta::Meta(Meta::NameValue(nv)) = nested {
						if nv.path.is_ident("name") {
							if let Lit::Str(lit) = &nv.lit {
								cap_name = lit.value();
							}
						} else if nv.path.is_ident("description") {
							if let Lit::Str(lit) = &nv.lit {
								description = lit.value();
							}
						}
					}
				}
			}
		}
	}

	// If cap_name is not set, use the struct name
	if cap_name.is_empty() {
		cap_name = name.to_string();
	}

	// Generate the new() method based on struct fields
	let new_method = match input.data {
		syn::Data::Struct(ref data_struct) => match data_struct.fields {
			Fields::Named(ref fields) => {
				let field_names = fields
					.named
					.iter()
					.map(|f| &f.ident)
					.cloned()
					.collect::<Vec<_>>();
				quote! {
					pub fn new(#(#field_names: _,)*) -> Self {
						Self { #(#field_names,)* }
					}
				}
			}
			Fields::Unnamed(_) => {
				quote! {
					pub fn new() -> Self {
						Self()
					}
				}
			}
			Fields::Unit => {
				quote! {
					pub fn new() -> Self {
						Self
					}
				}
			}
		},
		_ => quote! {},
	};

	let expanded = quote! {
		impl #name {
			#new_method
		}

		impl Capability for #name {
			fn name(&self) -> &'static str {
				#cap_name
			}

			fn description(&self) -> &'static str {
				#description
			}
		}
	};

	TokenStream::from(expanded)
}
