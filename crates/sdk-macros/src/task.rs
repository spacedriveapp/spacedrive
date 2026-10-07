//! Task macro implementation
//!
//! `#[task(retries = 2, timeout_ms = 30000)]` on an async function makes a
//! unit struct of the function's name that implements `spacedrive_sdk::Task`,
//! so `ctx.run(detect_faces, photo)` passes the policy along with the code.
//! The body keeps running as a renamed function the trait impl calls.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
	parse::{Parse, ParseStream},
	parse_macro_input, FnArg, GenericArgument, ItemFn, Lit, PathArguments, ReturnType, Token, Type,
};

struct TaskArgs {
	retries: u32,
	timeout_ms: u64,
}

impl Parse for TaskArgs {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let mut args = TaskArgs {
			retries: 0,
			timeout_ms: 0,
		};
		while !input.is_empty() {
			let key: syn::Ident = input.parse()?;
			input.parse::<Token![=]>()?;
			let value: Lit = input.parse()?;
			match (key.to_string().as_str(), value) {
				("retries", Lit::Int(n)) => args.retries = n.base10_parse()?,
				("timeout_ms", Lit::Int(n)) => args.timeout_ms = n.base10_parse()?,
				// Capability routing has no host side; the attribute is kept
				// for documentation.
				("requires_capability", Lit::Str(_)) => {}
				(other, _) => {
					return Err(syn::Error::new(
						key.span(),
						format!("unknown task attribute `{other}`"),
					))
				}
			}
			if input.peek(Token![,]) {
				input.parse::<Token![,]>()?;
			}
		}
		Ok(args)
	}
}

pub fn task_impl(args: TokenStream, input: TokenStream) -> TokenStream {
	let args = parse_macro_input!(args as TaskArgs);
	let mut input_fn = parse_macro_input!(input as ItemFn);

	let name = input_fn.sig.ident.clone();
	let name_str = name.to_string();
	let vis = input_fn.vis.clone();
	let docs: Vec<_> = input_fn
		.attrs
		.iter()
		.filter(|attr| attr.path().is_ident("doc"))
		.cloned()
		.collect();
	let inner = format_ident!("__task_{}", name);
	input_fn.sig.ident = inner.clone();

	let args_type = match input_fn.sig.inputs.iter().nth(1) {
		Some(FnArg::Typed(pat)) => (*pat.ty).clone(),
		_ => {
			return syn::Error::new_spanned(
				&input_fn.sig,
				"a task takes (ctx: TaskContext, args: A)",
			)
			.to_compile_error()
			.into()
		}
	};
	let output_type = match &input_fn.sig.output {
		ReturnType::Type(_, ty) => result_ok_type(ty),
		ReturnType::Default => None,
	};
	let Some(output_type) = output_type else {
		return syn::Error::new_spanned(&input_fn.sig.output, "a task returns TaskResult<T>")
			.to_compile_error()
			.into();
	};
	let retries = args.retries;
	let timeout_ms = args.timeout_ms;

	let expanded = quote! {
		#input_fn

		#(#docs)*
		#[allow(non_camel_case_types)]
		#[derive(Clone, Copy)]
		#vis struct #name;

		impl ::spacedrive_sdk::tasks::Task for #name {
			type Args = #args_type;
			type Output = #output_type;
			const NAME: &'static str = #name_str;
			const RETRIES: u32 = #retries;
			const TIMEOUT_MS: u64 = #timeout_ms;

			async fn call(
				ctx: ::spacedrive_sdk::tasks::TaskContext,
				args: Self::Args,
			) -> ::spacedrive_sdk::tasks::TaskResult<Self::Output> {
				#inner(ctx, args).await
			}
		}
	};

	TokenStream::from(expanded)
}

/// `T` out of `TaskResult<T>` or `Result<T>` or `Result<T, E>`.
fn result_ok_type(ty: &Type) -> Option<Type> {
	let Type::Path(path) = ty else {
		return None;
	};
	let last = path.path.segments.last()?;
	let PathArguments::AngleBracketed(generics) = &last.arguments else {
		return None;
	};
	match generics.args.first()? {
		GenericArgument::Type(ok) => Some(ok.clone()),
		_ => None,
	}
}
