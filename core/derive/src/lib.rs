use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput};

/// This macro must be executed in a file with `PropertyOperationCtx` defined and in the same package as the SyncContext is defined.
/// The creates:
/// ```rust
/// impl PropertyOperation {
///   fn apply(operation: PropertyOperationCtx, ctx: SyncContext) {
///     match operation.resource {
///       PropertyOperation::Tag(method) => method.apply(ctx),
///     };
///   }
/// }
/// ```
#[proc_macro_derive(PropertyOperationApply)]
pub fn property_operation_apply(input: TokenStream) -> TokenStream {
	let DeriveInput { ident, data, .. } = parse_macro_input!(input);

	if let Data::Enum(data) = data {
		let impls = data.variants.iter().map(|variant| {
			let variant_ident = &variant.ident;
			quote! {
			  #ident::#variant_ident(method) => method.apply(ctx),
			}
		});

		let expanded = quote! {
		  impl #ident {
			fn apply(operation: CrdtCtx<PropertyOperation>, ctx: self::engine::SyncContext) {
			  match operation.resource {
				#(#impls)*
			  };
			}
		  }
		};

		TokenStream::from(expanded)
	} else {
		panic!("The 'PropertyOperationApply' macro can only be used on enums!");
	}
}
