use crate::prelude::*;

pub fn r#struct(datamodel: &dml::Datamodel) -> TokenStream {
	let model_action_fns = datamodel.models.iter().map(|model| {
		let model_name_snake = snake_ident(&model.name);
		let model_actions_struct = quote!(super::#model_name_snake::Actions);

		quote! {
			pub fn #model_name_snake(&self) -> #model_actions_struct {
				#model_actions_struct { client: self }
			}
		}
	});

	quote! {
		pub struct PrismaCRDTClient {
			pub(super) client: #PRISMA::PrismaClient,
			pub node_id: Vec<u8>,
			operation_sender: #MPSC::Sender<#SYNC::CRDTOperation>
		}

		impl PrismaCRDTClient {
			pub(super) fn _new(
				client: #PRISMA::PrismaClient,
				node_id: Vec<u8>,
				operation_sender: #MPSC::Sender<#SYNC::CRDTOperation>
			) -> Self {
				Self {
					client,
					node_id,
					operation_sender,
				}
			}

			#(#model_action_fns)*
		}
	}
}
