use prisma_client_rust_sdk::{prelude::*, prisma::prisma_models::walkers::RefinedFieldWalker};

use crate::{ModelSyncType, ModelWithSyncType};

pub fn module((model, sync_type): ModelWithSyncType) -> Module {
	let model_name_snake = snake_ident(model.name());

	let sync_id = sync_type.as_ref().map(|sync_type| {
		let fields = sync_type.sync_id();
		let fields = fields.iter().flat_map(|field| {
			let name_snake = snake_ident(field.name());

			let typ = match field.refine() {
				RefinedFieldWalker::Scalar(_) => field.type_tokens(&quote!(self)),
				RefinedFieldWalker::Relation(relation) => {
					let relation_model_name_snake = snake_ident(relation.related_model().name());
					Some(quote!(super::#relation_model_name_snake::SyncId))
				}
			};

			Some(quote!(pub #name_snake: #typ))
		});

		let model_stuff = match sync_type {
			ModelSyncType::Relation {
				item,
				group,
				model_id,
			} => {
				let item_name_snake = snake_ident(item.name());
				let item_model_name_snake = snake_ident(item.related_model().name());

				let group_name_snake = snake_ident(group.name());
				let group_model_name_snake = snake_ident(group.related_model().name());

				Some(quote! {
					impl sd_sync::RelationSyncId for SyncId {
						type ItemSyncId = super::#item_model_name_snake::SyncId;
						type GroupSyncId = super::#group_model_name_snake::SyncId;

						fn split(&self) -> (&Self::ItemSyncId, &Self::GroupSyncId) {
							(
								&self.#item_name_snake,
								&self.#group_name_snake
							)
						}
					}

					pub const MODEL_ID: u16 = #model_id;

					impl sd_sync::SyncModel for #model_name_snake::Types {
						const MODEL_ID: u16 = MODEL_ID;
					}

					impl sd_sync::RelationSyncModel for #model_name_snake::Types {
						type SyncId = SyncId;
					}
				})
			}
			ModelSyncType::Shared { model_id, .. } => Some(quote! {
					pub const MODEL_ID: u16 = #model_id;

					impl sd_sync::SyncModel for #model_name_snake::Types {
						const MODEL_ID: u16 = MODEL_ID;
					}

					impl sd_sync::SharedSyncModel for #model_name_snake::Types {
					  type SyncId = SyncId;
					}
			}),
			_ => None,
		};

		quote! {
			#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
			pub struct SyncId {
				#(#fields),*
			}

			impl sd_sync::SyncId for SyncId {
				type Model = #model_name_snake::Types;
			}

			#model_stuff
		}
	});

	let set_param_impl = {
		let field_matches = model.fields().filter_map(|field| {
			let field_name_snake = snake_ident(field.name());

			match field.refine() {
				RefinedFieldWalker::Scalar(scalar_field) => {
					(!scalar_field.is_in_required_relation()).then(|| {
						quote! {
							#model_name_snake::#field_name_snake::set(::rmpv::ext::from_value(val).unwrap()),
						}
					})
				}
				RefinedFieldWalker::Relation(relation_field) => {
					let relation_model_name_snake =
						snake_ident(relation_field.related_model().name());

					match relation_field.referenced_fields() {
						Some(i) => {
							if i.count() == 1 {
								Some(quote! {{
									let val: std::collections::HashMap<String, rmpv::Value> = ::rmpv::ext::from_value(val).unwrap();
									let val = val.into_iter().next().unwrap();

									#model_name_snake::#field_name_snake::connect(
										#relation_model_name_snake::UniqueWhereParam::deserialize(&val.0, val.1).unwrap()
									)
								}})
							} else {
								None
							}
						}
						_ => None,
					}
				}
			}
			.map(|body| quote!(#model_name_snake::#field_name_snake::NAME => #body))
		});

		match field_matches.clone().count() {
			0 => quote!(),
			_ => quote! {
				impl #model_name_snake::SetParam {
					pub fn deserialize(field: &str, val: ::rmpv::Value) -> Option<Self> {
						Some(match field {
							#(#field_matches)*
							_ => return None
						})
					}
				}
			},
		}
	};

	let unique_param_impl = {
		let field_matches = model
			.unique_criterias()
			.flat_map(|criteria| match &criteria.fields().next() {
				Some(field) if criteria.fields().len() == 1 => {
					let field_name_snake = snake_ident(field.name());

					Some(quote!(#model_name_snake::#field_name_snake::NAME =>
						#model_name_snake::#field_name_snake::equals(
							::rmpv::ext::from_value(val).unwrap()
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
					pub fn deserialize(field: &str, val: ::rmpv::Value) -> Option<Self> {
						Some(match field {
							#(#field_matches)*
							_ => return None
						})
					}
				}
			},
		}
	};

	Module::new(
		model.name(),
		quote! {
			use super::prisma::*;
			use prisma_client_rust::scalar_types::*;

			#sync_id

			#set_param_impl

			#unique_param_impl
		},
	)
}
