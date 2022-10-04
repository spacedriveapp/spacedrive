use crate::generator::prelude::*;

/// Constructs the Sync ID for the given model.
///
/// ## Scalar Example
/// ```
/// @shared(id: unique_id)
/// model User {
///     pk Int   @id
///     id Bytes @unique
/// }
/// ```
///
/// ```
/// SyncID {
///     id: #data_var.id
/// }
/// ```
///
/// ## Relation Example
///
/// ```
/// @shared(id: [location, pk])
/// model File {
///     pk Int @id
///
///     location_id Int
///     location    Location
/// }
/// ```
///
/// ```
/// SyncID {
///     location_id: self
///         .client
///         .client
///         .file()
///         .find_unique(crate::prisma::location::local_id::equals(#data_var.location_id.clone()))
///         .exec()
///         .await?
///         .id,
///     pk: #data_var.pk.clone()
/// }
/// ```
pub fn constructor(model: ModelRef, data_var: TokenStream) -> TokenStream {
	let model_name_snake = snake_ident(&model.name);

	let args = model
		.fields()
		.into_iter()
		.filter(|f| model.is_sync_id(f.name()))
		.flat_map(|f| match &f.typ {
			FieldType::Scalar { .. } => vec![f],
			FieldType::Relation { relation_info } => relation_info
				.fields
				.iter()
				.map(|f| {
					model
						.field(f)
						.unwrap_or_else(|| panic!("{} has no field {}", model.name, f))
				})
				.collect(),
		})
		.map(|f| {
			let field_name_snake = snake_ident(f.name());

			let val = scalar_field_to_crdt(
				f,
				quote!(self.crdt_client.client),
				quote!(#data_var.#field_name_snake),
			);

			quote!(#field_name_snake: #val)
		});

	quote! {
		super::#model_name_snake::SyncID {
			#(#args,)*
		}
	}
}

/// Generates tokens to get the CRDT value of a scalar field.
///
/// ## Reguar Field
/// For a field that has no connection to any model's sync ID,
/// the value used will be `set_param_value`.
///
/// ```
/// field String
/// ```
///
/// ```
/// #set_param_value
/// ```
///
/// ## Sync ID Field
/// For a field that is a foreign key, a query fetching the foreign model's
/// corresponding Sync ID will be generated.
///
/// ```
/// relation_id String
/// relation    RelationModel @relation(fields: [relation_id], references: [foreign_pk])
/// ````
///
/// ```
/// #client
///     .relation()
///     .find_unique(crate::prisma::relation::id::equals(#set_param_value))
///     .exec()
///     .await
///     .unwrap()
///     .unwrap()
///     .foregin_sync_id
/// ```

pub fn scalar_field_to_crdt(
	field: FieldRef,
	client: TokenStream,
	set_param_value: TokenStream,
) -> TokenStream {
	match &field.typ {
		FieldType::Scalar {
			relation_field_info,
		} => relation_field_info
			.as_ref()
			.and_then(|relation_field_info| {
				let referenced_field_snake = snake_ident(relation_field_info.referenced_field);

				let relation_model = field
					.datamodel
					.model(relation_field_info.referenced_model)
					.unwrap();
				let relation_model_snake = snake_ident(&relation_model.name);

				let referenced_sync_id_field = relation_model
					.sync_id_for_pk(relation_field_info.referenced_field)
					.expect("referenced_sync_id_field should be present");

				// If referenced field is a sync ID, it does not need to be converted
				(!field.model.is_sync_id(relation_field_info.referenced_field)).then(|| {
					let referenced_sync_id_field_name_snake =
						snake_ident(referenced_sync_id_field.name());

					let query = quote! {
						#client
							.#relation_model_snake()
							.find_unique(
								crate::prisma::#relation_model_snake::#referenced_field_snake::equals(#set_param_value)
							)
							.exec()
							.await
							.unwrap()
							.unwrap()
							.#referenced_sync_id_field_name_snake
					};

					match field.arity() {
						dml::FieldArity::Optional => {
							quote! {
								// can't map with async :sob:
								match #set_param_value {
									Some(#set_param_value) => Some(#query),
									None => None,
								}
							}
						}
						_ => query,
					}
				})
			})
			.unwrap_or(quote!(#set_param_value)),
		_ => unreachable!(),
	}
}

/// Generates a definition of a model's `SyncID` struct
///
/// ## Example
///
/// ```
/// #[derive(Clone, ::serde::Serialize, ::serde::Deserialize)]
/// pub struct SyncID {
///     pub id: i32,
///     pub location_id: Vec<u8>
/// }
/// ```
pub fn definition(model: ModelRef) -> TokenStream {
	let sync_id_fields = model.scalar_sync_id_fields(&model.datamodel).map(|field| {
		let field_type = field.1.crdt_type_tokens(&model.datamodel);
		let field_name_snake = snake_ident(field.0.name());

		quote!(#field_name_snake: #field_type)
	});

	quote! {
		#[derive(Clone, ::serde::Serialize, ::serde::Deserialize)]
		pub struct SyncID {
			#(pub #sync_id_fields),*
		}
	}
}
