#![allow(dead_code)]

// use sdcore::{prisma, sync::engine::test, sync::FakeCoreContext};

use std::fs::File;

fn main() {
	let file = File::open("/Users/james/Desktop/Cloud/preview.mp4").unwrap();

	println!("{:?}", file.metadata().unwrap())
}
