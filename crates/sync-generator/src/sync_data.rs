use prisma_client_rust_sdk::{
	prelude::*,
	prisma::prisma_models::walkers::{RefinedFieldWalker, RelationFieldWalker},
};
use prisma_models::walkers::{FieldWalker, ScalarFieldWalker};

use crate::{ModelSyncType, ModelWithSyncType};

pub fn enumerate(models: &[ModelWithSyncType<'_>]) -> TokenStream {
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
							#model_name_snake::MODEL_ID =>
								Self::#model_name_pascal(rmpv::ext::from_value(op.record_id).ok()?, op.data)
						},
					)
				})
		})
		.unzip();

	let exec_matches = models.iter().filter_map(|(model, sync_type)| {
		let model_name_pascal = pascal_ident(model.name());
		let model_name_snake = snake_ident(model.name());

		let match_arms = match sync_type.as_ref()? {
			ModelSyncType::Shared { id, model_id } => {
				handle_crdt_ops_shared(id, *model_id, &model_name_snake)
			}
			ModelSyncType::Relation { item, group, .. } => {
				handle_crdt_ops_relation(models, item, group, &model_name_snake)
			}
			ModelSyncType::Local { .. } => return None,
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
				Some(match op.model {
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

fn handle_crdt_ops_relation(
	models: &[ModelWithSyncType<'_>],
	item: &RelationFieldWalker<'_>,
	group: &RelationFieldWalker<'_>,
	model_name_snake: &Ident,
) -> TokenStream {
	let compound_id = format_ident!(
		"{}",
		group
			.fields()
			.expect("missing group fields")
			.chain(item.fields().expect("missing item fields"))
			.map(ScalarFieldWalker::name)
			.collect::<Vec<_>>()
			.join("_")
	);

	let db_batch_items = {
		let batch_item = |item: &RelationFieldWalker<'_>| {
			let item_model_sync_id_field_name_snake = models
				.iter()
				.find(|m| m.0.name() == item.related_model().name())
				.and_then(|(_m, sync)| sync.as_ref())
				.map(|sync| snake_ident(sync.sync_id()[0].name()))
				.expect("missing sync id field name for relation");
			let item_model_name_snake = snake_ident(item.related_model().name());
			let item_field_name_snake = snake_ident(item.name());

			quote! {
				db.#item_model_name_snake()
					.find_unique(
						prisma::#item_model_name_snake::#item_model_sync_id_field_name_snake::equals(
							id.#item_field_name_snake.#item_model_sync_id_field_name_snake.clone()
						)
					)
					.select(prisma::#item_model_name_snake::select!({ id }))
			}
		};

		[batch_item(group), batch_item(item)]
	};

	let create_items = {
		let create_item = |item: &RelationFieldWalker<'_>, var: TokenStream| {
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
		let (Some(group), Some(item)) =
			(#(#db_batch_items.exec().await?),*) else {
				panic!("item and group not found!");
		};

		let id = prisma::#model_name_snake::#compound_id(group.id, item.id);

		match data {
			sd_sync::CRDTOperationData::Create(_) => {
				db.#model_name_snake()
					.upsert(
						id,
						prisma::#model_name_snake::create(
							#(#create_items),*,
							vec![]
						),
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

#[inline]
fn handle_crdt_ops_shared(
	id: &FieldWalker<'_>,
	model_id: u16,
	model_name_snake: &Ident,
) -> TokenStream {
	let (get_id, equals_value, id_name_snake, create_id) = match id.refine() {
		RefinedFieldWalker::Relation(rel) => {
			let scalar_field = rel
				.fields()
				.expect("missing fields")
				.next()
				.expect("empty fields");
			let id_name_snake = snake_ident(scalar_field.name());
			let field_name_snake = snake_ident(rel.name());
			let opposite_model_name_snake = snake_ident(
				rel.opposite_relation_field()
					.expect("missing opposite relation field")
					.model()
					.name(),
			);

			let relation_equals_condition = quote!(prisma::#opposite_model_name_snake::pub_id::equals(
			   id.#field_name_snake.pub_id.clone()
			));

			let rel_fetch = quote! {
				let rel = db.#opposite_model_name_snake()
					.find_unique(#relation_equals_condition)
					.exec()
					.await?
					.unwrap();
			};

			(
				Some(rel_fetch),
				quote!(rel.id),
				id_name_snake,
				relation_equals_condition,
			)
		}
		RefinedFieldWalker::Scalar(s) => {
			let field_name_snake = snake_ident(s.name());
			let thing = quote!(id.#field_name_snake.clone());

			(None, thing.clone(), field_name_snake, thing)
		}
	};

	quote! {
		#get_id

		match data {
			sd_sync::CRDTOperationData::Create(data) => {
				let data: Vec<_> = data.into_iter().map(|(field, value)| {
					prisma::#model_name_snake::SetParam::deserialize(&field, value).unwrap()
				}).collect();

				db.#model_name_snake()
					.upsert(
						prisma::#model_name_snake::#id_name_snake::equals(#equals_value),
						prisma::#model_name_snake::create(#create_id, data.clone()),
						data
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
						prisma::#model_name_snake::#id_name_snake::equals(#equals_value),
						prisma::#model_name_snake::create(#create_id, data.clone()),
						data,
					)
					.exec()
					.await?;
			},
			sd_sync::CRDTOperationData::Delete => {
				db.#model_name_snake()
						.delete(prisma::#model_name_snake::#id_name_snake::equals(#equals_value))
						.exec()
						.await?;

				db.crdt_operation()
					.delete_many(vec![
						prisma::crdt_operation::model::equals(#model_id as i32),
						prisma::crdt_operation::record_id::equals(rmp_serde::to_vec(&id).unwrap()),
						prisma::crdt_operation::kind::equals(sd_sync::OperationKind::Create.to_string())
					])
					.exec()
					.await?;
			},
		}
	}
}
