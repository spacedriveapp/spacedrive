mod client;
mod model;

use super::prelude::*;
use super::*;

#[derive(Deserialize)]
pub struct PrismaCRDTGenerator {}

impl PrismaGenerator for PrismaCRDTGenerator {
	const NAME: &'static str = "Prisma CRDT Generator";
	const DEFAULT_OUTPUT: &'static str = "./prisma-crdt.rs";

	fn generate(self, args: GenerateArgs) -> String {
		let datamodel =
			datamodel::Datamodel::try_from(&args.dml).expect("Failed to construct datamodel");
		let datamodel_ref = prelude::DatamodelRef(&datamodel);

		let header = quote! {
			#![allow(clippy::all)]

			pub async fn new_client(
				prisma_client: crate::prisma::PrismaClient,
				node_id: Vec<u8>,
				node_local_id: i32
			) -> (
				_prisma::PrismaCRDTClient,
				::tokio::sync::mpsc::Receiver<::prisma_crdt::CRDTOperation>,
			) {
				let (tx, rx) = ::tokio::sync::mpsc::channel(64);

				let crdt_client = _prisma::PrismaCRDTClient::_new(prisma_client, (node_id, node_local_id), tx);
				(crdt_client, rx)
			}
			pub use _prisma::*;
		};

		let client = client::generate(datamodel_ref);

		let models = datamodel
			.models
			.iter()
			.map(|model| model::generate(ModelRef::new(model, datamodel_ref)));

		let output = quote! {
			#header

			#(#models)*

			#client
		};

		output.to_string()
	}
}
