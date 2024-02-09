use prisma_client_rust_sdk::{prelude::*, prisma::prisma_models::walkers::RelationFieldWalker};

use crate::{ModelSyncType, ModelWithSyncType};

pub fn r#enum(models: Vec<ModelWithSyncType>) -> TokenStream {
	let (variants, matches): (Vec<_>, Vec<_>) = models
		.iter()
		.filter_map(|(model, sync_type)| {
			let model_name_snake = snake_ident(model.name());
			let model_name_pascal = pascal_ident(model.name());

			sync_type
				.as_ref()
				.filter(|s| {
					matches!(
						s,
						ModelSyncType::Shared { .. } | ModelSyncType::Relation { .. }
					)
				})
				.map(|_| {
					(
						quote!(#model_name_pascal(#model_name_snake::SyncId, sd_sync::CRDTOperationData)),
						quote! {
							prisma::#model_name_snake::NAME =>
								Self::#model_name_pascal(serde_json::from_value(op.record_id).ok()?, op.data)
						},
					)
				})
		})
		.unzip();

	let exec_matches = models.iter().filter_map(|(model, sync_type)| {
		let model_name_pascal = pascal_ident(model.name());
		let model_name_snake = snake_ident(model.name());

		let match_arms = match sync_type.as_ref()? {
			ModelSyncType::Shared { id } => {
				let id_name_snake = snake_ident(id.name());

				quote! {
					match data {
						sd_sync::CRDTOperationData::Create => {
							db.#model_name_snake()
								.upsert(
									prisma::#model_name_snake::#id_name_snake::equals(id.#id_name_snake.clone()),
									prisma::#model_name_snake::create(id.#id_name_snake, vec![]),
									vec![]
								)
								.exec()
								.await?;
						},
						sd_sync::CRDTOperationData::Update { field, value } => {
							let data = vec![
								prisma::#model_name_snake::SetParam::deserialize(&field, value).unwrap()
							];

							db.#model_name_snake()
								.upsert(
									prisma::#model_name_snake::#id_name_snake::equals(id.#id_name_snake.clone()),
									prisma::#model_name_snake::create(id.#id_name_snake, data.clone()),
									data,
								)
								.exec()
								.await?;
						},
						sd_sync::CRDTOperationData::Delete => {
							db.#model_name_snake()
									.delete(prisma::#model_name_snake::#id_name_snake::equals(id.#id_name_snake))
									.exec()
									.await?;
						},
					}
				}
			}
			ModelSyncType::Relation { item, group } => {
				let compound_id = format_ident!(
					"{}",
					item.fields()
						.unwrap()
						.chain(group.fields().unwrap())
						.map(|f| f.name())
						.collect::<Vec<_>>()
						.join("_")
				);

				let db_batch_items = {
					let batch_item = |item: &RelationFieldWalker| {
						let item_model_sync_id_field_name_snake = models
							.iter()
							.find(|m| m.0.name() == item.related_model().name())
							.and_then(|(m, sync)| sync.as_ref())
							.map(|sync| snake_ident(sync.sync_id()[0].name()))
							.unwrap();
						let item_model_name_snake = snake_ident(item.related_model().name());
						let item_field_name_snake = snake_ident(item.name());

						quote!(db.#item_model_name_snake().find_unique(
							prisma::#item_model_name_snake::#item_model_sync_id_field_name_snake::equals(
								id.#item_field_name_snake.#item_model_sync_id_field_name_snake.clone()
							)
						))
					};

					[batch_item(item), batch_item(group)]
				};

				let create_items = {
					let create_item = |item: &RelationFieldWalker, var: TokenStream| {
						let item_model_name_snake = snake_ident(item.related_model().name());

						quote!(
							prisma::#item_model_name_snake::id::equals(#var.id)
						)
					};

					[
						create_item(item, quote!(item)),
						create_item(group, quote!(group)),
					]
				};

				quote! {
					let (Some(item), Some(group)) =
						db._batch((#(#db_batch_items),*)).await? else {
							panic!("item and group not found!");
					};

					let id = prisma::#model_name_snake::#compound_id(item.id, group.id);

					match data {
						sd_sync::CRDTOperationData::Create => {
							db.#model_name_snake()
								.create(
									#(#create_items),*,
									vec![],
								)
								.exec()
								.await
								.ok();
						},
						sd_sync::CRDTOperationData::Update { field, value } => {
							let data = vec![prisma::#model_name_snake::SetParam::deserialize(&field, value).unwrap()];

							db.#model_name_snake()
								.upsert(
									id,
									prisma::#model_name_snake::create(
										#(#create_items),*,
										data.clone(),
									),
									data,
								)
								.exec()
								.await
								.ok();
						},
						sd_sync::CRDTOperationData::Delete => {
							db.#model_name_snake()
								.delete(id)
								.exec()
								.await
								.ok();
						},
					}
				}
			}
			_ => return None,
		};

		Some(quote! {
			Self::#model_name_pascal(id, data) => {
				#match_arms
			}
		})
	});

	quote! {
		pub enum ModelSyncData {
			#(#variants),*
		}

		impl ModelSyncData {
			pub fn from_op(op: sd_sync::CRDTOperation) -> Option<Self> {
				Some(match op.model.as_str() {
					#(#matches),*,
					_ => return None
				})
			}

			pub async fn exec(self, db: &prisma::PrismaClient) -> prisma_client_rust::Result<()> {
				match self {
					#(#exec_matches),*
				}

				Ok(())
			}
		}
	}
}
