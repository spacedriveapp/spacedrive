//! Model macro implementation - generates ExtensionModel trait impl
//! Handles field attributes: #[entry], #[sidecar], #[custom_field], #[computed], etc.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Attribute, Data, DeriveInput, Fields};

pub fn model_impl(_args: TokenStream, input: TokenStream) -> TokenStream {
	let mut input = parse_macro_input!(input as DeriveInput);

	// Find the id/uuid field before modifying
	let uuid_field = find_uuid_field(&input);
	let name = input.ident.clone();

	// Strip known field attributes (they'll be processed later when macros are enhanced)
	strip_field_attributes(&mut input);

	let expanded = quote! {
		#input

		impl ::spacedrive_sdk::models::ExtensionModel for #name {
			const MODEL_TYPE: &'static str = stringify!(#name);

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
				// Remove known field attributes (for now - will be processed in future)
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
