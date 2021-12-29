use std::path::Path;

use hotwatch::{Event, Hotwatch};

pub async fn watch_dir(path: &str) {
    let mut watcher = Hotwatch::new().expect("hotwatch failed to initialize!");

    watcher.watch(Path::new(path), move |event: Event| {
        println!("hotwatch event: {:?}", event);
    });
    // .expect(format!("failed to watch directory {}", &path).as_str());

    println!("watching directory {:?}", Path::new(&path));
}
