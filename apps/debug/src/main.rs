#![allow(dead_code)]

pub mod checksum;

use sdcore::{prisma, sync::engine::test, sync::FakeCoreContext};

#[tokio::main]
async fn main() {
	let db = prisma::new_client().await.unwrap();
	let ctx = FakeCoreContext {};
	test(&ctx).await;
	// checksum::do_thing();

	// create an HLC with a generated UUID and relying on SystemTime::now()
	// let hlc = HLC::default();

	// // generate timestamps
	// let ts1 = hlc.new_timestamp();
	// let ts2 = hlc.new_timestamp();

	// println!("ts1 {}", ts1);
	// println!("ts2 {}", ts2);
}
