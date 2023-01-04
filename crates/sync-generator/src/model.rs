use crate::{attribute::Attribute, prelude::*};

pub enum ModelType {
	Owned,
	SharedUnique,
	SharedAtomic,
	Relation,
}

pub fn model_sync_type(model: &dml::Model, dml: &dml::Datamodel) -> ModelType {
	let type_attribute = model
		.documentation
		.as_ref()
		.map(Attribute::parse)
		.unwrap()
		.unwrap();

	match type_attribute.name {
		"owned" => ModelType::Owned,
		"shared" => ModelType::SharedUnique, // TODO: fix
		"relation" => ModelType::Relation,
		_ => unreachable!(),
	}
}
pub fn module(model: &dml::Model, dml: &dml::Datamodel) -> TokenStream {
	let model_name_snake = snake_ident(&model.name);

	let set_params_enum = set_params_enum(model, dml);

	let actions_struct = actions_struct(model, dml);

	quote! {
		pub mod #model_name_snake {
			#set_params_enum

			#actions_struct
		}
	}
}

pub fn set_params_enum(model: &dml::Model, dml: &dml::Datamodel) -> TokenStream {
	quote! {
		pub enum SetParam {}
	}
}

pub fn create_fn(model: &dml::Model, dml: &dml::Datamodel) -> TokenStream {
	let required_scalar_fields = model.required_scalar_fields();

	let args = required_scalar_fields.iter().map(|field| {
		let name = snake_ident(field.name());
		let typ = field.type_tokens(quote!(crate::prisma::));

		quote!(#name: #typ)
	});

	match model_sync_type(model, dml) {
		ModelType::Owned => {
			quote! {
				pub fn create(&self, #(#args),*, _params: Vec<SetParam>) {

				}
			}
		}
		ModelType::SharedUnique => {
			quote! {
				pub fn create(&self, #(#args),*, _params: Vec<SetParam>) {}
			}
		}
		ModelType::SharedAtomic => {
			quote! {
				pub fn create(&self, _params: Vec<SetParam>) {}
			}
		}
		ModelType::Relation => {
			quote! {
				pub fn create(&self, _params: Vec<SetParam>) {}
			}
		}
		_ => todo!(),
	}
}

pub fn actions_struct(model: &dml::Model, dml: &dml::Datamodel) -> TokenStream {
	let create_fn = create_fn(model, dml);

	quote! {
		pub struct Actions<'a> {
			pub(super) client: &'a super::#CRDT_CLIENT
		}

		impl<'a> Actions<'a> {
			#create_fn
		}
	}
}
