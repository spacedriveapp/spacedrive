use std::path::Path;

use hotwatch::{
	blocking::{Flow, Hotwatch},
	Event,
};

pub fn watch_dir(path: &str) {
	let mut hotwatch = Hotwatch::new().expect("hotwatch failed to initialize!");
	hotwatch
		.watch(&path, |event: Event| {
			if let Event::Write(path) = event {
				println!("{:?} changed!", path);
				// Flow::Exit
				Flow::Continue
			} else {
				Flow::Continue
			}
		})
		.expect("failed to watch file!");

	hotwatch.run();

	println!("watching directory {:?}", Path::new(&path));
}
