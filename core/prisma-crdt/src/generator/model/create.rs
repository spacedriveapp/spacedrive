use crate::generator::prelude::*;

use super::{sync_id::{self, ScalarFieldToCRDT}, set_param, relation::RelationKeyConstructor};

struct CRDTParamsConstructor<'a> {
	model: &'a Model<'a>,
	datamodel: &'a Datamodel<'a>,
}

impl ToTokens for CRDTParamsConstructor<'_> {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		let crdt_create_params = self
            .model
			.fields
			.iter()
			.filter(|f| f.is_scalar_field() &&
                f.required_on_create() && 
                self.model.scalar_sync_id_fields(self.datamodel).all(|(sf, _)| sf.name() != f.name())
            )
			.map(|field| {
				let field_name_snake = snake_ident(field.name());
				
                let value = ScalarFieldToCRDT::new(
                    field, 
                    self.model, 
                    self.datamodel,
                    quote!(self.client.client),
                    quote!(self.set_params.#field_name_snake)
                );
				
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
		
        let create_call = PrismaCreateCall {
            model: self.model
        };

		let sync_id_constructor = sync_id::constructor(&self.model, quote!(self.set_params), self.datamodel);
		
		let crdt_params_constructor = CRDTParamsConstructor {
			model: self.model,
			datamodel: self.datamodel,
		};
	
		tokens.extend(quote! {
			pub async fn exec(
				self,
			) -> Result<crate::prisma::#model_name::Data, crate::prisma::QueryError> {
                let res = #create_call;		

                let sync_id = #sync_id_constructor;
				
                let params = #crdt_params_constructor;
				
                let params_map = ::prisma_crdt::objectify(params);

                self
                   .client
                    ._create_operation(::prisma_crdt::CRDTOperationType::owned(
                        #model_name_str,
                        vec![::prisma_crdt::OwnedOperationData::Create(params_map)]
                    ))
                    .await;
                
                Ok(res)
            }
		});
	}
}

struct SharedCreateExec<'a> {
	model: &'a Model<'a>,
	datamodel: &'a Datamodel<'a>
}

impl<'a> ToTokens for SharedCreateExec<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let model_name = snake_ident(&self.model.name);
        let model_name_str = &self.model.name;
        
        let sync_id_constructor = sync_id::constructor(&self.model, quote!(self.set_params), self.datamodel);
        
        let create_call = PrismaCreateCall {
            model: self.model
        };

        let create_mode = match &self.model.typ {
            ModelType::Shared { create, .. } => create,
            _ => unreachable!()
        };

        let the_meat = match create_mode {
            SharedCreateType::Atomic => {
                quote! {
                    self
                        .client
                        ._create_operation(::prisma_crdt::CRDTOperationType::shared(
                            #model_name_str,
                            ::serde_json::to_value(&sync_id).unwrap(),
                            ::prisma_crdt::SharedOperationData::create_atomic()
                        ))
                        .await;

                    for param in self.set_params._params {
                        let crdt_param = param.into_crdt(self.client).await;

                        let param_map = ::prisma_crdt::objectify(crdt_param);

                        for (key, value) in param_map {
                            self
                                .client
                                ._create_operation(::prisma_crdt::CRDTOperationType::shared(
                                    #model_name_str,
                                    ::serde_json::to_value(&sync_id).unwrap(),
                                    ::prisma_crdt::SharedOperationData::update(key, value)
                                ))
                                .await;
                        }
                    }
                }
            },
            SharedCreateType::Unique => {
                let crdt_params_constructor = CRDTParamsConstructor {
                    model: self.model,
                    datamodel: self.datamodel,
                };
        
                quote! {
                    let params = #crdt_params_constructor;
                    
                    let params_map = ::prisma_crdt::objectify(params);

                    self
                        .client
                        ._create_operation(::prisma_crdt::CRDTOperationType::shared(
                            #model_name_str,
                            ::serde_json::to_value(&sync_id).unwrap(),
                            ::prisma_crdt::SharedOperationData::create_unique(params_map)
                        ))
                        .await;
                }
            }
        };
        
        tokens.extend(quote! {
            pub async fn exec(
                self,
            ) -> Result<crate::prisma::#model_name::Data, crate::prisma::QueryError> {
                let res = #create_call;

                let sync_id = #sync_id_constructor;

                #the_meat
                
                Ok(res)
            }
        }); 
    }
}

struct RelationCreateExec<'a> {
	model: &'a Model<'a>,
	datamodel: &'a Datamodel<'a>
}

impl ToTokens for RelationCreateExec<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) { 
        let model_name = snake_ident(&self.model.name);
        let model_name_str = &self.model.name;

        let create_call = PrismaCreateCall {
            model: self.model
        };

        let (relation_item_block, relation_group_block) = match &self.model.typ {
            ModelType::Relation { item, group } => {
                let relation_item_block = RelationKeyConstructor::new(
                    self.model.field(item.at_index(0).unwrap()).unwrap(),
                    self.model,
                    self.datamodel,
                    quote!(RelationItem)
                );
                let relation_group_block = RelationKeyConstructor::new(
                    self.model.field(group.at_index(0).unwrap()).unwrap(),
                    self.model,
                    self.datamodel,
                    quote!(RelationGroup)
                ); 

                (relation_item_block, relation_group_block)
            },
            _ => unreachable!()
        };

        tokens.extend(quote! {
            pub async fn exec(
                self,
            ) -> Result<crate::prisma::#model_name::Data, crate::prisma::QueryError> {
                let res = #create_call;

                let relation_item = #relation_item_block;
                
                let relation_group = #relation_group_block;

                self
                    .client
                    ._create_operation(::prisma_crdt::CRDTOperationType::relation(
                        #model_name_str,
                        ::serde_json::to_vec(&relation_item).unwrap(),
                        ::serde_json::to_vec(&relation_group).unwrap(),
                        ::prisma_crdt::RelationOperationData::create()
                    ))
                    .await;

                Ok(res)
            }
        })
    }
}

enum CreateExec<'a> {
    Owned(OwnedCreateExec<'a>),
    Shared(SharedCreateExec<'a>),
    Relation(RelationCreateExec<'a>)
}

impl<'a> ToTokens for CreateExec<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            CreateExec::Owned(owned) => owned.to_tokens(tokens),
            CreateExec::Shared(shared) => shared.to_tokens(tokens),
            CreateExec::Relation(relation) => relation.to_tokens(tokens)
        }
    }
}

pub fn generate<'a>(model: &'a Model<'a>, datamodel: &'a Datamodel<'a>) -> TokenStream {
	let model_name_snake = snake_ident(&model.name);
	

    let exec = match &model.typ {
        ModelType::Owned { .. } => CreateExec::Owned(OwnedCreateExec {
            model,
            datamodel
        }),
        ModelType::Shared { .. } => CreateExec::Shared(SharedCreateExec {
            model,
            datamodel
        }),
        ModelType::Relation { .. } => CreateExec::Relation(RelationCreateExec {
            model,
            datamodel
        }),
        _ => CreateExec::Owned(OwnedCreateExec {
            model,
            datamodel
        })
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
