pub use prisma_client_rust_sdk::prelude::*;

macro_rules! impl_quote {
	($struct:ident = $tokens:expr) => {
		#[allow(non_camel_case_types)]
		pub struct $struct;

		impl ToTokens for $struct {
			fn to_tokens(&self, tokens: &mut TokenStream) {
				tokens.extend(quote!($tokens));
			}
		}
	};
}

impl_quote!(PRISMA = crate::prisma);
impl_quote!(SYNC = ::sd_sync);
impl_quote!(MPSC = ::tokio::sync::mpsc);
impl_quote!(CRDT_CLIENT = _prisma::PrismaCRDTClient);
