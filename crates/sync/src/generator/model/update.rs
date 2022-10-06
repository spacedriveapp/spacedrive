use crate::generator::prelude::*;

pub fn generate(model: &Model) -> TokenStream {
	let model_name_snake = snake_ident(&model.name);

	quote! {
		#[derive(serde::Serialize, serde::Deserialize)]
		struct CRDTUpdateParams {
			#[serde(default, skip_serializing_if = "Vec::is_empty", rename = "_")]
			pub _params: Vec<CRDTSetParam>,
			#[serde(flatten)]
			pub _sync_id: SyncID,
		}

		pub struct Update<'a> {
			client: &'a super::_prisma::PrismaCRDTClient,
			where_param: crate::prisma::#model_name_snake::UniqueWhereParam,
			set_params: Vec<SetParam>,
		}

		impl <'a> Update<'a> {
			pub async fn exec(self) -> Result<Option<crate::prisma::#model_name_snake::Data>, crate::prisma::QueryError> {

			}
		}
	}
}
