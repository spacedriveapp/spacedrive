pub use super::datamodel::*;
pub use prisma_client_rust_sdk::prisma_datamodel::dml;
pub use prisma_client_rust_sdk::*;
pub use proc_macro2::*;
pub use quote::*;
use std::ops::Deref;

pub fn snake_ident(name: &str) -> Ident {
	format_ident!("{}", name.to_case(Case::Snake))
}

pub fn pascal_ident(name: &str) -> Ident {
	format_ident!("{}", name.to_case(Case::Pascal))
}

pub struct ModelRef<'a> {
	model: &'a Model<'a>,
	datamodel: &'a Datamodel<'a>,
}

impl<'a> Deref for ModelRef<'a> {
	type Target = Model<'a>;

	fn deref(&self) -> &Self::Target {
		&self.model
	}
}

pub struct FieldRef<'a> {
	field: &'a Field<'a>,
	model: &'a Model<'a>,
	datamodel: &'a Datamodel<'a>,
}

impl<'a> Deref for FieldRef<'a> {
	type Target = Field<'a>;

	fn deref(&self) -> &Self::Target {
		&self.field
	}
}