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
	fn from_attribute(attr: Attribute, model: &'a dml::Model) -> Option<Self> {
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

impl ToTokens for ModelSyncType<'_> {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		let variant = match self {
			Self::Local { .. } => "Local",
			Self::Owned { .. } => "Owned",
			Self::Shared { .. } => "Shared",
			Self::Relation { .. } => "Relation",
		};

		tokens.append(format_ident!("{variant}SyncType"));
	}
}

impl PrismaGenerator for SDSyncGenerator {
	const NAME: &'static str = "SD Sync Generator";
	const DEFAULT_OUTPUT: &'static str = "prisma-sync.rs";

	type Error = Error;

	fn generate(self, args: GenerateArgs) -> Result<String, Self::Error> {
		let models_with_attributes = args
			.dml
			.models()
			.map(|model| (model, model_attributes(model)));

		let models_with_sync_types = models_with_attributes.map(|(model, attributes)| {
			let sync_type = attributes
				.into_iter()
				.find_map(|a| ModelSyncType::from_attribute(a, model));

			(model, sync_type)
		});

		let model_modules = models_with_sync_types.clone().map(|(model, sync_type)| {
			let model_name_snake = snake_ident(&model.name);

            let sync_id = sync_type.as_ref()
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
                            type Marker = sd_sync::#sync_type;
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

		let model_sync_data = {
			let (variants, matches): (Vec<_>, Vec<_>) = models_with_sync_types
				.filter_map(|(model, sync_type)| {
					let model_name_snake = snake_ident(&model.name);
					let model_name_pascal = pascal_ident(&model.name);
					let model_name_str = &model.name;

					sync_type.and_then(|a| {
						let data_type = match a {
							ModelSyncType::Owned { .. } => quote!(OwnedOperationData),
							ModelSyncType::Shared { .. } => quote!(SharedOperationData),
							ModelSyncType::Relation { .. } => {
								quote!(RelationOperationData)
							}
							_ => return None,
						};

						let variant = quote! {
							#model_name_pascal(#model_name_snake::SyncId, sd_sync::#data_type)
						};

						let op_type_enum = quote!(sd_sync::CRDTOperationType);

						let match_case = match a {
							ModelSyncType::Owned { .. } => {
								quote! {
									#op_type_enum::Owned(op) if op.model == #model_name_str =>
										Self::#model_name_pascal(serde_json::from_value(op.record_id).ok()?, op.data)
								}
							}
							ModelSyncType::Shared { .. } => {
								quote! {
									#op_type_enum::Shared(op) if op.model == #model_name_str =>
										Self::#model_name_pascal(serde_json::from_value(op.record_id).ok()?, op.data)
								}
							}
							// ModelSyncType::Relation { .. } => {
							// 	quote! {
							// 		(#model_name_str, sd_sync::CRDTOperation::Relation(op)) =>
							// 			Self::#model_name_pascal()
							// 	}
							// }
							_ => return None,
						};

						Some((variant, match_case))
					})
				})
				.unzip();

			quote! {
				pub enum ModelSyncData {
					#(#variants),*
				}

				impl ModelSyncData {
					pub fn from_op(op: sd_sync::CRDTOperationType) -> Option<Self> {
						Some(match op {
							#(#matches),*,
							_ => return None
						})
					}
				}
			}
		};

		Ok(quote! {
			use crate::prisma;

			#model_sync_data

			#(#model_modules)*
		}
		.to_string())
	}
}

pub fn run() {
	SDSyncGenerator::run();
}
