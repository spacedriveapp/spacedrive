use crate::generator::prelude::*;

use super::sync_id;

struct CRDTParamsConstructor<'a> {
	model: &'a Model<'a>,
	datamodel: &'a Datamodel<'a>,
	sync_id_var: &'a str,
}

impl ToTokens for CRDTParamsConstructor<'_> {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		let sync_id_val = self.sync_id_var;
		let model = self.model;
		
		let crdt_create_params = model
			.fields
			.iter()
			.filter(|f| f.is_scalar_field() && f.required_on_create() && model.scalar_sync_id_fields(self.datamodel).all(|(sf, _)| sf.name() != f.name()))
			.map(|field| {
				let field_name_snake = snake_ident(field.name());
				
				let value = match &field.typ {
					FieldType::Scalar { relation_field_info } => {
						match relation_field_info {
							Some(relation_field_info) => {
								let relation_name_snake = snake_ident(relation_field_info.relation);
								let relation_model = self.datamodel.model(&relation_field_info
									.referenced_model)
									.unwrap();
									
								let referenced_field_name_snake = snake_ident(relation_model
									.sync_id_for_pk(&relation_field_info.referenced_field)
									.map(|f| f.name())
									.unwrap_or(relation_field_info.referenced_field));
								
								quote!(res.#relation_name_snake().unwrap().#referenced_field_name_snake.clone())
							},
							None => quote!(res.#field_name_snake.clone()),	
						}
					},
					_ => unreachable!()
				};
				
				quote!(#field_name_snake: #value)
			});
		
		tokens.extend(quote! {
			CRDTCreateParams {
				_params: {
                    let mut params = vec![];

                    for _param in self.set_params._params {
                        params.push(_param.into_crdt(&self.client).await);
                    }

                    params
                },
				_sync_id: sync_id.clone(),
				#(#crdt_create_params,)*
			};
		});
	}
}

struct PrismaCreateCall<'a> {
	model: &'a Model<'a>,
}

impl ToTokens for PrismaCreateCall<'_> {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		let model_name = snake_ident(&self.model.name);
		
		let create_args = self.model
			.fields
			.iter()
			.filter(|f| {
				f.required_on_create()
					&& f.as_scalar_field()
						.map(|sf| !self.model.scalar_field_has_relation(sf))
						.unwrap_or(true)
			})
			.map(|field| {
				let field_name_snake = snake_ident(field.name());

				match &field.typ {
					FieldType::Relation {relation_info} => {						
						let relation_model_snake = snake_ident(relation_info.to);
						
						if relation_info.fields.len() == 1 {						
							let relation_field_snake = snake_ident(&relation_info.fields[0]);
							let referenced_field_snake = snake_ident(&relation_info.references[0]);
							
							quote!(crate::prisma::#relation_model_snake::#referenced_field_snake::equals(self.set_params.#relation_field_snake.clone()))
						} else {
							todo!()
						}

					},
					_ => quote!(self.set_params.#field_name_snake.clone()),
				}
			});
		
		tokens.extend(quote! {
			self
				.client
				.client
				.#model_name()
				.create(
					#(#create_args,)*
					self.set_params._params.clone().into_iter().map(Into::into).collect(),
				)
				.exec()
				.await?;
		});
	}
}

struct OwnedCreateExec<'a> {
	model: &'a Model<'a>,
	datamodel: &'a Datamodel<'a>
}

impl ToTokens for OwnedCreateExec<'_> {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		let model_name = snake_ident(&self.model.name);
		let model_name_str = &self.model.name;
		
		let sync_id_constructor = sync_id::constructor(&self.model, quote!(self.set_params), self.datamodel);
		
		let crdt_params_constructor = CRDTParamsConstructor {
			model: self.model,
			datamodel: self.datamodel,
			sync_id_var: "res"
		};
		
		let create_call = PrismaCreateCall {
			model: self.model
		};
		
		tokens.extend(quote! {
			pub async fn exec(
				self,
			) -> Result<crate::prisma::#model_name::Data, crate::prisma::QueryError> {
				let sync_id = #sync_id_constructor;

				let res = #create_call;
				
                let params = #crdt_params_constructor;

                let params_map = match serde_json::to_value(params).unwrap() {
                	serde_json::Value::Object(m) => m,
                	_ => unreachable!(),
                };
				
				panic!()
			}
		});
	}
}

pub fn generate<'a>(model: &'a Model<'a>, datamodel: &'a Datamodel<'a>) -> TokenStream {
	let model_name_snake = snake_ident(&model.name);
	
	let exec = OwnedCreateExec {
		model: &model,
		datamodel: &datamodel
	};

	quote! {
		pub struct Create<'a> {
			client: &'a super::_prisma::PrismaCRDTClient,
			set_params: CreateParams,
			with_params: Vec<crate::prisma::#model_name_snake::WithParam>,
		}

		impl<'a> Create<'a> {
			pub fn with(mut self, param: impl Into<crate::prisma::#model_name_snake::WithParam>) -> Self {
				self.with_params.push(param.into());
				self
			}
			
			#exec
		}
	}
}
