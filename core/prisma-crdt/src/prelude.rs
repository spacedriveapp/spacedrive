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

#[derive(Clone, Copy)]
pub struct DatamodelRef<'a>(pub &'a Datamodel<'a>);

impl<'a> DatamodelRef<'a> {
    pub fn models(&self) -> Vec<ModelRef<'a>> {
        self.0.models.iter().map(|m| ModelRef::new(m, *self)).collect()
    }
}

impl<'a> Deref for DatamodelRef<'a> {
    type Target = Datamodel<'a>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone, Copy)]
pub struct ModelRef<'a> {
	model: &'a Model<'a>,
	pub datamodel: DatamodelRef<'a>,
}

impl<'a> ModelRef<'a> {
    pub fn new(model: &'a Model<'a>, datamodel: DatamodelRef<'a>) -> Self {
        ModelRef { model, datamodel }
    }

    pub fn fields(&self) -> Vec<FieldRef<'a>> {
        self.model.fields.iter().map(|f| FieldRef::new(f, *self)).collect()
    }

    pub fn field(&self, name: &str) -> Option<FieldRef> {
        self.model.fields.iter().find(|f| f.name() == name).map(|field| {
            FieldRef::new(field, *self)
        })
    }
}

impl<'a> Deref for ModelRef<'a> {
	type Target = Model<'a>;

	fn deref(&self) -> &Self::Target {
		&self.model
	}
}

#[derive(Clone, Copy)]
pub struct FieldRef<'a> {
	field: &'a Field<'a>,
	pub model: ModelRef<'a>,
	pub datamodel: DatamodelRef<'a>,
}

impl<'a> FieldRef<'a> {
    pub fn new(field: &'a Field<'a>, model: ModelRef<'a>) -> Self {
        FieldRef { field, model, datamodel: model.datamodel }
    }
}

impl<'a> Deref for FieldRef<'a> {
	type Target = Field<'a>;

	fn deref(&self) -> &Self::Target {
		&self.field
	}
}
