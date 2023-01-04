// mod model;
mod prelude;

use prelude::*;

#[derive(Debug, serde::Serialize, thiserror::Error)]
enum Error {}

#[derive(serde::Deserialize)]
struct SDSyncGenerator {}

impl PrismaGenerator for SDSyncGenerator {
	const NAME: &'static str = "SD Sync Generator";
	const DEFAULT_OUTPUT: &'static str = "prisma-sync.rs";

	type Error = Error;

	fn generate(self, args: GenerateArgs) -> Result<String, Self::Error> {
		let set_param_impls = args.dml.models().map(|model| {
			let model_name_snake = snake_ident(&model.name);

			let field_matches = model.fields().filter_map(|field| {
				let field_name_snake = snake_ident(field.name());
				let field_name_snake_str = field_name_snake.to_string();

                match field {
                    dml::Field::ScalarField(_) => {
                        Some(quote! {
                            #field_name_snake_str => crate::prisma::#model_name_snake::#field_name_snake::set(::serde_json::from_value(val).unwrap()),
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

                                        crate::prisma::#model_name_snake::#field_name_snake::connect(
                                            crate::prisma::#relation_model_name_snake::UniqueWhereParam::deserialize(&val.0, val.1).unwrap()
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
                    impl crate::prisma::#model_name_snake::SetParam {
                        pub fn deserialize(field: &str, val: ::serde_json::Value) -> Option<Self> {
                            Some(match field {
                                #(#field_matches)*
                                _ => return None
                            })
                        }
                    }
                }
            }
		});

		let unique_where_param_impls = args.dml.models().map(|model| {
			let model_name_snake = snake_ident(&model.name);

			let field_matches = model
				.loose_unique_criterias()
				.iter()
				.flat_map(|criteria| match &criteria.fields[..] {
					[field] => {
						let unique_field_name_str = &field.name;
						let unique_field_name_snake = snake_ident(&unique_field_name_str);

						Some(quote!(#unique_field_name_str =>
							crate::prisma::#model_name_snake::#unique_field_name_snake::equals(
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
                    impl crate::prisma::#model_name_snake::UniqueWhereParam {
                        pub fn deserialize(field: &str, val: ::serde_json::Value) -> Option<Self> {
                            Some(match field {
                                #(#field_matches)*
                                _ => return None
                            })
                        }
                    }
                }
            }
		});

		Ok(quote! {
			#(#set_param_impls)*

			#(#unique_where_param_impls)*
		}
		.to_string())
	}
}

pub fn run() {
	SDSyncGenerator::run();
}
