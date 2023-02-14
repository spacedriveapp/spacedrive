mod attribute;

use attribute::*;

use prisma_client_rust_sdk::prelude::*;

#[derive(Debug, serde::Serialize, thiserror::Error)]
enum Error {}

#[derive(serde::Deserialize)]
struct SDSyncGenerator {}

type FieldVec<'a> = Vec<&'a dml::Field>;

#[derive(Debug)]
#[allow(unused)]
enum ModelSyncType<'a> {
	Local {
		id: FieldVec<'a>,
	},
	Owned {
		id: FieldVec<'a>,
	},
	Shared {
		id: FieldVec<'a>,
	},
	Relation {
		group: FieldVec<'a>,
		item: FieldVec<'a>,
	},
}

impl<'a> ModelSyncType<'a> {
	fn from_attribute(attr: &'a Attribute, model: &'a dml::Model) -> Option<Self> {
		let id = attr
			.field("id")
			.map(|field| match field {
				AttributeFieldValue::Single(s) => vec![*s],
				AttributeFieldValue::List(l) => l.clone(),
			})
			.unwrap_or_else(|| {
				model
					.primary_key
					.as_ref()
					.unwrap()
					.fields
					.iter()
					.map(|f| f.name.as_str())
					.collect()
			})
			.into_iter()
			.flat_map(|name| model.find_field(name))
			.collect();

		Some(match attr.name {
			"local" => Self::Local { id },
			"owned" => Self::Owned { id },
			"shared" => Self::Shared { id },
			_ => return None,
		})
	}

	fn sync_id(&self) -> Vec<&dml::Field> {
		match self {
			Self::Owned { id } => id.clone(),
			Self::Local { id } => id.clone(),
			Self::Shared { id } => id.clone(),
			_ => vec![],
		}
	}
}

impl PrismaGenerator for SDSyncGenerator {
	const NAME: &'static str = "SD Sync Generator";
	const DEFAULT_OUTPUT: &'static str = "prisma-sync.rs";

	type Error = Error;

	fn generate(self, args: GenerateArgs) -> Result<String, Self::Error> {
		let model_modules = args.dml.models().map(|model| {
            let model_name_snake = snake_ident(&model.name);

            let attributes = model_attributes(model);

            let sync_id = attributes
                .iter()
                .find_map(|a| ModelSyncType::from_attribute(a, model))
                .map(|sync_type| {
                    let fields = sync_type.sync_id();
                    let fields = fields.iter().flat_map(|field| {
                        let name_snake = snake_ident(field.name());

                        let typ = match field {
                            dml::Field::ScalarField(_) => {
                                field.type_tokens(quote!(self))
                            },
                            dml::Field::RelationField(relation)=> {
                                let relation_model_name_snake = snake_ident(&relation.relation_info.referenced_model);
                                quote!(super::#relation_model_name_snake::SyncId)
                            },
                            _ => return None
                        };

                        Some(quote!(pub #name_snake: #typ))
                    });

                    let sync_type_marker = match &sync_type {
                        ModelSyncType::Local { .. } => quote!(LocalSyncType),
                        ModelSyncType::Owned { .. } => quote!(OwnedSyncType),
                        ModelSyncType::Shared { .. } => quote!(SharedSyncType),
                        ModelSyncType::Relation { .. } => quote!(RelationSyncType),
                    };

                    quote! {
                        #[derive(serde::Serialize, serde::Deserialize)]
                        pub struct SyncId {
                            #(#fields),*
                        }

                        impl sd_sync::SyncId for SyncId {
                            type ModelTypes = #model_name_snake::Types;
                        }

                        impl sd_sync::SyncType for #model_name_snake::Types {
                            type SyncId = SyncId;
                            type Marker = sd_sync::#sync_type_marker;
                        }
                    }
                });

            let set_param_impl = {
                let field_matches = model.fields().filter_map(|field| {
                    let field_name_snake = snake_ident(field.name());
                    let field_name_snake_str = field_name_snake.to_string();


                    match field {
                        dml::Field::ScalarField(_) => {
                            Some(quote! {
                                #field_name_snake_str => #model_name_snake::#field_name_snake::set(::serde_json::from_value(val).unwrap()),
                            })
                        },
                        dml::Field::RelationField(relation_field) => {
                            let relation_model_name_snake = snake_ident(&relation_field.relation_info.referenced_model);

                            match &relation_field.relation_info.references[..] {
                                [_] => {
                                    Some(quote! {
                                        #field_name_snake_str => {
                                            let val: std::collections::HashMap<String, ::serde_json::Value> = ::serde_json::from_value(val).unwrap();
                                            let val = val.into_iter().next().unwrap();

                                            #model_name_snake::#field_name_snake::connect(
                                                #relation_model_name_snake::UniqueWhereParam::deserialize(&val.0, val.1).unwrap()
                                            )
                                        },
                                    })
                                },
                                _ => None
                            }
                        },
                        _ => None
                    }
                });

                match field_matches.clone().count() {
                    0 => quote!(),
                    _ => quote! {
                        impl #model_name_snake::SetParam {
                            pub fn deserialize(field: &str, val: ::serde_json::Value) -> Option<Self> {
                                Some(match field {
                                    #(#field_matches)*
                                    _ => return None
                                })
                            }
                        }
                    }
                }
            };

            let unique_param_impl = {
                let field_matches = model
                    .loose_unique_criterias()
                    .iter()
                    .flat_map(|criteria| match &criteria.fields[..] {
                        [field] => {
                            let unique_field_name_str = &field.name;
                            let unique_field_name_snake = snake_ident(unique_field_name_str);

                            Some(quote!(#unique_field_name_str =>
                                #model_name_snake::#unique_field_name_snake::equals(
                                    ::serde_json::from_value(val).unwrap()
                                ),
                            ))
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();

                match field_matches.len() {
                    0 => quote!(),
                    _ => quote! {
                        impl #model_name_snake::UniqueWhereParam {
                            pub fn deserialize(field: &str, val: ::serde_json::Value) -> Option<Self> {
                                Some(match field {
                                    #(#field_matches)*
                                    _ => return None
                                })
                            }
                        }
                    },
                }
            };

            quote! {
                pub mod #model_name_snake {
                    use super::prisma::*;

                    #sync_id

                    #set_param_impl

                    #unique_param_impl
                }
            }
        });

		Ok(quote! {
			use crate::prisma;

			#(#model_modules)*
		}
		.to_string())
	}
}

pub fn run() {
	SDSyncGenerator::run();
}
