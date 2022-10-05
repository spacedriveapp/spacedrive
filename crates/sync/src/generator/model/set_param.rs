use crate::generator::prelude::*;

use super::sync_id::scalar_field_to_crdt;

struct SetParam {
	pub variant: TokenStream,
	pub into_match_arm: TokenStream,
	pub into_crdt_match_arm: TokenStream,
	pub from_pcr_set_impl: TokenStream,
}

impl SetParam {
	pub fn new(field: FieldRef) -> Self {
		let model_name_snake = snake_ident(&field.model.name);
		let field_name_snake = snake_ident(field.name());
		let field_name_pascal = pascal_ident(field.name());

		let variant_name = format_ident!("Set{}", field_name_pascal);

		let variant = {
			let variant_type = field.type_tokens();
			quote!(#variant_name(#variant_type))
		};

		let into_match_arm = quote!(Self::#variant_name(v) => crate::prisma::#model_name_snake::#field_name_snake::set(v));

		let into_crdt_match_arm = {
			let to_crdt_block = scalar_field_to_crdt(field, quote!(client), quote!(v));

			quote!(Self::#variant_name(v) => CRDTSetParam::#variant_name(#to_crdt_block))
		};

		let from_pcr_set_impl = quote! {
			impl From<crate::prisma::#model_name_snake::#field_name_snake::Set> for SetParam {
				fn from(v: crate::prisma::#model_name_snake::#field_name_snake::Set) -> Self {
					Self::#variant_name(v.0)
				}
			}
		};

		SetParam {
			variant,
			into_match_arm,
			into_crdt_match_arm,
			from_pcr_set_impl,
		}
	}
}

struct CRDTSetParam {
	pub variant: TokenStream,
	pub into_match_arm: TokenStream,
}

impl CRDTSetParam {
	pub fn new(field: FieldRef) -> Self {
		let model_name_snake = snake_ident(&field.model.name);
		let field_name_snake = snake_ident(field.name());
		let field_name_pascal = pascal_ident(field.name());

		let variant_name = format_ident!("Set{}", field_name_pascal);

		let variant = {
			let variant_type = field.crdt_type_tokens(&field.datamodel);
			let field_name = field.name();

			quote! {
				#[serde(rename = #field_name)]
				#variant_name(#variant_type)
			}
		};

		let into_match_arm = {
			let relation_field_info = match &field.typ {
				FieldType::Scalar {
					relation_field_info,
				} => relation_field_info,
				_ => unreachable!("Cannot create CRDTSetParam from relation field!"),
			};

			let ret = match relation_field_info.as_ref() {
				Some(relation_field_info)
                    if
						field.model.name != relation_field_info.referenced_model // This probably isn't good enough
					 =>
				{
					let relation_name_snake = snake_ident(relation_field_info.relation);
					let relation_model = field.datamodel.model(relation_field_info
						.referenced_model).unwrap();
					let relation_model_name_snake = snake_ident(&relation_model.name);

					let referenced_sync_id_field = relation_model
						.sync_id_for_pk(relation_field_info.referenced_field)
						.expect("referenced_sync_id_field should be present");
					let referenced_sync_id_field_name_snake = snake_ident(referenced_sync_id_field.name());

					let ret = quote!(crate::prisma::#model_name_snake::#relation_name_snake::link(
						crate::prisma::#relation_model_name_snake::#referenced_sync_id_field_name_snake::equals(v)
					));

					match field.arity() {
						dml::FieldArity::Optional => {
							quote!(v.map(|v| #ret).unwrap_or(crate::prisma::#model_name_snake::#relation_name_snake::unlink()))
						}
						_ => ret,
					}
				}
				_ => {
					quote!(crate::prisma::#model_name_snake::#field_name_snake::set(v))
				}
			};
			quote!(Self::#variant_name(v) => #ret)
		};

		Self {
			variant,
			into_match_arm,
		}
	}
}

pub fn definition(model: ModelRef) -> TokenStream {
	let model_name_snake = snake_ident(&model.name);

	let set_param_fields_iter = model.fields().into_iter().filter(|f| {
		model
			.scalar_sync_id_fields(&model.datamodel)
			.any(|(id, _)| id.name() == f.name())
			|| f.is_scalar_field() && !(model.is_pk(f.name()) || model.is_sync_id(f.name()))
	});

	let set_params = set_param_fields_iter.clone().map(SetParam::new);

	let set_param_variants = set_params.clone().map(|p| p.variant);
	let set_param_into_match_arms = set_params.clone().map(|p| p.into_match_arm);
	let set_param_into_crdt_match_arms = set_params.clone().map(|p| p.into_crdt_match_arm);
	let set_param_from_pcr_set_impls = set_params.clone().map(|p| p.from_pcr_set_impl);

	let crdt_set_params = set_param_fields_iter.map(CRDTSetParam::new);

	let crdt_set_param_variants = crdt_set_params.clone().map(|p| p.variant);
	let crdt_set_param_into_match_arms = crdt_set_params.clone().map(|p| p.into_match_arm);

	quote! {
		#[derive(Clone)]
		pub enum SetParam {
			#(#set_param_variants),*
		}

		impl SetParam {
			pub async fn into_crdt(self, client: &super::_prisma::PrismaCRDTClient) -> CRDTSetParam {
				match self {
					#(#set_param_into_crdt_match_arms),*
				}
			}
		}

		#(#set_param_from_pcr_set_impls)*

		impl Into<crate::prisma::#model_name_snake::SetParam> for SetParam {
			fn into(self) -> crate::prisma::#model_name_snake::SetParam {
				match self {
					#(#set_param_into_match_arms),*
				}
			}
		}

		#[derive(Clone, serde::Serialize, serde::Deserialize)]
		pub enum CRDTSetParam {
			#(#crdt_set_param_variants),*
		}

		impl Into<crate::prisma::#model_name_snake::SetParam> for CRDTSetParam {
			fn into(self) -> crate::prisma::#model_name_snake::SetParam {
				match self {
					#(#crdt_set_param_into_match_arms),*
				}
			}
		}
	}
}
