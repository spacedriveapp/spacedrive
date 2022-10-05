// use crate::generator::prelude::*;

// pub fn generate(model: &Model) -> TokenStream {
// 	let model_name = snake_ident(&model.name);
//
// 	quote! {
// 		pub struct Delete<'a> {
// 			client: &'a super::_prisma::PrismaCRDTClient,
// 			where_param: crate::prisma::#model_name::UniqueWhereParam,
// 			with_params: Vec<crate::prisma::#model_name::WithParam>,
// 		}
//
// 		impl<'a> Delete<'a> {
//     		pub fn with(mut self, param: impl Into<crate::prisma::location::WithParam>) -> Self {
//     			self.with_params.push(param.into());
//     			self
//     		}
//
//     		pub async fn exec(self) -> Result<Option<crate::prisma::#model_name::Data>, crate::prisma::QueryError> {
//
//     		}
// 		}
// 	}
// }
