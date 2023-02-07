use crate::generator::prelude::*;

use super::{create_params, sync_id};

/// Generates the body of an owned model's `Create::exec` function
///
/// ## Example
///
/// ```
/// let sync_id = SyncId { .. };
///
/// let params = CRDTCreateParams { .. };
///
/// let params_map = ::prisma_crdt::objectify(params);
///
/// self
///     .crdt_client
///     .create_operation(::prisma_crdt::CRDTOperationType::owned(
///         #model_name_str,
///         vec![::prisma_crdt::OwnedOperationData::Create(params_map)]
///     ))
///     .await;
/// ```
pub fn create_exec_body(model: ModelRef) -> TokenStream {
	let model_name_str = &model.name;

	let sync_id_constructor = sync_id::constructor(model, quote!(self.set_params));

	let crdt_params_constructor = create_params::crdt_constructor(model);

	quote! {
		let sync_id = #sync_id_constructor;

		let params = #crdt_params_constructor;

		let params_map = ::prisma_crdt::objectify(params);

		self
		   .crdt_client
			._create_operation(::prisma_crdt::CRDTOperationType::owned(
				#model_name_str,
				vec![::prisma_crdt::OwnedOperationData::Create(params_map)]
			))
			.await;
	}
}
