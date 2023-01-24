use crate::generator::prelude::*;

use super::{create_params, sync_id};

/// Generates the body of a shared relation's `Create::exec` function
///
/// ## Example
///
/// ### Atomic
///
/// ```
/// let sync_id = SyncId { .. };
///
/// self
///     .crdt_client
///     ._create_operation(::prisma_crdt::CRDTOperationType::shared(
///         #model_name_str,
///         ::serde_json::to_value(&sync_id).unwrap(),
///         ::prisma_crdt::SharedOperationData::create_atomic()
///     ))
///     .await;
///
/// for param in self.set_params._params {
///     let crdt_param = param.into_crdt(self.crdt_client).await;
///
///     let param_map = ::prisma_crdt::objectify(crdt_param);
///
///     for (key, value) in param_map {
///         self
///             .crdt_client
///             ._create_operation(::prisma_crdt::CRDTOperation::shared(
///                 #model_name_str,
///                 ::serde_json::to_value(&sync_id).unwrap(),
///                 ::prisma_crdt::SharedOperationData::update(key, value)
///             ))
///             .await;
///     }
/// }
/// ```
///
/// ### Unique
///
/// ```
/// let sync_id = SyncId { .. };
///
/// let params = CreateCRDTParams { .. };
///
/// let params_map = ::prisma_crdt::objectify(params);
///
/// self
///     .crdt_client
///     ._create_operation(::prisma_crdt::CRDTOperationType::shared(
///         #model_name_str,
///         ::serde_json::to_value(&sync_id).unwrap(),
///         ::prisma_crdt::SharedOperationData::create_unique(params)
///     ))
///     .await;
/// ```
pub fn create_exec_body(model: ModelRef) -> TokenStream {
	let model_name_str = &model.name;

	let sync_id_constructor = sync_id::constructor(model, quote!(self.set_params));

	let create_mode = match &model.typ {
		ModelType::Shared { create, .. } => create,
		_ => unreachable!(),
	};

	let the_meat = match create_mode {
		SharedCreateType::Atomic => {
			quote! {
				self
					.crdt_client
					._create_operation(::prisma_crdt::CRDTOperationType::shared(
						#model_name_str,
						::serde_json::to_value(&sync_id).unwrap(),
						::prisma_crdt::SharedOperationData::create_atomic()
					))
					.await;

				for param in self.set_params._params {
					let crdt_param = param.into_crdt(self.crdt_client).await;

					let param_map = ::prisma_crdt::objectify(crdt_param);

					for (key, value) in param_map {
						self
							.crdt_client
							._create_operation(::prisma_crdt::CRDTOperationType::shared(
								#model_name_str,
								::serde_json::to_value(&sync_id).unwrap(),
								::prisma_crdt::SharedOperationData::update(key, value)
							))
							.await;
					}
				}
			}
		}
		SharedCreateType::Unique => {
			let crdt_params_constructor = create_params::crdt_constructor(model);

			quote! {
				let params = #crdt_params_constructor;

				let params_map = ::prisma_crdt::objectify(params);

				self
					.crdt_client
					._create_operation(::prisma_crdt::CRDTOperationType::shared(
						#model_name_str,
						::serde_json::to_value(&sync_id).unwrap(),
						::prisma_crdt::SharedOperationData::create_unique(params_map)
					))
					.await;
			}
		}
	};

	quote! {
		let sync_id = #sync_id_constructor;

		#the_meat
	}
}
