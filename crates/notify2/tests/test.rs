use std::{env, fs, path::PathBuf, process::Command, time::Duration};

use notify2::{Event, Type, Watcher};
use tokio::{sync::mpsc, time::sleep};
use utils::{set_readyonly, TestDir};

mod utils;

async fn basic_tests(dir: &PathBuf, rx: &mut mpsc::Receiver<Event>) {
	let paths = [
		(dir.join("dir1"), Type::Directory),
		(dir.join("file1.txt"), Type::File),
		(dir.join("dir1").join("dir2"), Type::Directory),
		(dir.join("dir1").join("file2.txt"), Type::File),
	];

	// Creation
	for (path, ty) in paths.iter() {
		match ty {
			Type::Directory => fs::create_dir(&path),
			Type::File => fs::write(&path, b"Monty"),
			Type::Symlink => todo!(),  // TODO
			Type::Hardlink => todo!(), // TODO
		}
		.unwrap();
		sleep(Duration::from_millis(500)).await;

		assert_eq!(
			rx.recv().await,
			Some(Event::Create(ty.clone(), path.clone()))
		);
	}
	tokio::select! {
		event = rx.recv() => {
			assert!(false, "The receiver channel should not receive any more create events. Received event: {:?}", event);
		},
		_ = sleep(Duration::from_millis(500)) => {}
	}

	// Edit file content
	for (path, ty) in paths.iter() {
		match ty {
			Type::Directory => return, // Can't edit content of file
			Type::File => fs::write(&path, b"Monty2"),
			Type::Symlink => todo!(),  // TODO
			Type::Hardlink => todo!(), // TODO
		}
		.unwrap();

		assert_eq!(
			rx.recv().await,
			Some(Event::Modify(ty.clone(), path.clone()))
		);
	}
	tokio::select! {
		event = rx.recv() => {
			assert!(false, "The receiver channel should not receive any more edit file events. Received event: {:?}", event);
		},
		_ = sleep(Duration::from_millis(500)) => {}
	}

	// Edit file permissions
	for (path, ty) in paths.iter() {
		set_readyonly(&path, true);
		sleep(Duration::from_millis(500)).await; // We do this so the OS doesn't deduplicate the event
		set_readyonly(&path, false);

		assert_eq!(
			rx.recv().await,
			Some(Event::Modify(ty.clone(), path.clone()))
		);
		assert_eq!(
			rx.recv().await,
			Some(Event::Modify(ty.clone(), path.clone()))
		);
	}
	tokio::select! {
		event = rx.recv() => {
			assert!(false, "The receiver channel should not receive any more edit file permissions file events. Received event: {:?}", event);
		},
		_ = sleep(Duration::from_millis(500)) => {}
	}

	// Move/rename & Delete
	// NOTE: `paths` is reversed because file metadata requires it to exist
	for (path, ty) in paths.into_iter().rev() {
		let new_path = path.with_file_name(format!(
			"{}-new{}",
			path.file_stem().unwrap().to_str().unwrap(),
			path.extension()
				.map(|v| format!(".{}", v.to_str().unwrap()))
				.unwrap_or_default()
		));

		// Rename
		fs::rename(&path, &new_path).unwrap();
		assert_eq!(
			rx.recv().await,
			Some(Event::Move {
				ty: ty.clone(),
				from: path.clone(),
				to: new_path.clone()
			})
		);

		// Delete
		match ty {
			Type::Directory => fs::remove_dir(&new_path),
			Type::File => fs::remove_file(&new_path),
			Type::Symlink => todo!(),  // TODO
			Type::Hardlink => todo!(), // TODO
		}
		.unwrap();
		assert_eq!(rx.recv().await, Some(Event::Delete(ty, new_path)));
	}
	tokio::select! {
		event = rx.recv() => {
			assert!(false, "The receiver channel should not receive any more move or delete file events. Received event: {:?}", event);
		},
		_ = sleep(Duration::from_millis(500)) => {}
	}
}

#[tokio::test]
async fn remove_path() {
	let (dir, _handle) = TestDir::new().await;
	let (mut rx, watcher) = Watcher::new(vec![dir.clone()], false).await;
	basic_tests(&dir, &mut rx).await;

	// remove path
	watcher.remove_paths(vec![dir.clone()]).await;
	fs::write(dir.join("demo.txt"), b"Monty3").unwrap();
	tokio::select! {
		event = rx.recv() => {
			assert!(false, "The receiver channel should not receive any more events after the path is removed. Received event: {:?}", event);
		},
		_ = sleep(Duration::from_millis(500)) => {}
	}
}

#[tokio::test]
async fn empty_watcher() {
	let (mut rx, watcher) = Watcher::new(vec![], false).await;

	tokio::select! {
		event = rx.recv() => {
			match event {
				Some(event) => assert!(false, "The receiver channel should not receive any events while no paths are being watched. Received event: {:?}", event),
				None => assert!(false, "The receiver channel should stay open until the watcher is dropped"),
			}
			assert!(false, "The receiver channel should stay open until the watcher is dropped")
		},
		_ = tokio::time::sleep(std::time::Duration::from_secs(2)) => {}
	}

	drop(watcher);
	let event = rx.recv().await;
	assert!(
		event.is_none(),
		"The receiver channel should be closed after the watcher is dropped"
	);
}

#[tokio::test]
async fn add_path_then_remove() {
	let (dir, _handle) = TestDir::new().await;
	let (mut rx, watcher) = Watcher::new(vec![], false).await;
	fs::write(dir.join("demo-before.txt"), b"Monty").unwrap();
	tokio::select! {
		event = rx.recv() => {
			assert!(false, "The receiver channel should not receive any more events after the path is removed. Received event: {:?}", event);
		},
		_ = sleep(Duration::from_millis(500)) => {}
	}

	// add path
	watcher.add_paths(vec![dir.clone()]).await;
	basic_tests(&dir, &mut rx).await;

	// remove path
	watcher.remove_paths(vec![dir.clone()]).await;
	fs::write(dir.join("demo-after.txt"), b"Millie").unwrap();
	tokio::select! {
		event = rx.recv() => {
			assert!(false, "The receiver channel should not receive any more events after the path is removed. Received event: {:?}", event);
		},
		_ = sleep(Duration::from_millis(500)) => {}
	}
}

// TODO: This test fails because all events that happen while the path is removed are received when it is added back.
// #[tokio::test]
// async fn remove_path_then_add() {
// 	let (dir, _handle) = TestDir::new().await;
// 	let (mut rx, watcher) = Watcher::new(vec![dir.clone()], false).await;
// 	basic_tests(&dir, &mut rx).await;

// 	// remove path
// 	watcher.remove_paths(vec![dir.clone()]).await;
// 	fs::write(dir.join("demo-after.txt"), b"Millie").unwrap();
// 	tokio::select! {
// 		event = rx.recv() => {
// 			assert!(false, "The receiver channel should not receive any more events after the path is removed. Received event: {:?}", event);
// 		},
// 		_ = sleep(Duration::from_millis(500)) => {}
// 	}

// 	// clear left overs from first run of `basic_tests`.
// 	fs::remove_dir_all(&dir).unwrap();
// 	fs::create_dir(&dir).unwrap();
// 	sleep(Duration::from_millis(2000)).await;

// 	// add path
// 	watcher.add_paths(vec![dir.clone()]).await;
// 	basic_tests(&dir, &mut rx).await;
// }

#[tokio::test]
async fn double_add() {
	let (dir, _handle) = TestDir::new().await;
	let (mut rx, watcher) = Watcher::new(vec![dir.clone(), dir.clone()], false).await; // Attempt to create with dir twice
	watcher.add_paths(vec![dir.clone()]).await; // Attempt to add directory that is already being watched
	watcher
		.add_paths(vec![dir.join("dir2").clone(), dir.join("dir2").clone()])
		.await; // Attempt to add_paths the same dir twice
	basic_tests(&dir, &mut rx).await; // Ensure no duplicate events happen
}

// TODO: Test test sometimes causes the runloop channel to freeze
// #[tokio::test]
// async fn double_remove() {
// 	let (dir, _handle) = TestDir::new().await;
// 	let (mut rx, watcher) = Watcher::new(vec![dir.clone()], false).await;
// 	println!("DOUBLE REMOVE A");
// 	watcher.remove_paths(vec![dir.clone(), dir.clone()]).await; // Attempt to remove directory that is being watched twice

// 	fs::write(dir.join("demo.txt"), b"Millie").unwrap();
// 	sleep(Duration::from_millis(250)).await;

// 	tokio::select! {
// 		event = rx.recv() => {
// 			assert!(false, "The receiver channel should not receive any events once a directory is removed from the watcher. Received event: {:?}", event);
// 		},
// 		_ = sleep(Duration::from_millis(500)) => {}
// 	}
// }

#[tokio::test]
async fn ignore_events_from_current_process() {
	let (dir, _handle) = TestDir::new().await;
	let (mut rx, _watcher) = Watcher::new(vec![dir.clone()], true).await; // The true is the import part for this test

	// Event created by current process
	fs::write(dir.join("demo.txt"), b"Millie").unwrap();
	tokio::select! {
		event = rx.recv() => {
			assert!(false, "The receiver channel should not receive any events created by the current process. Received event: {:?}", event);
		},
		_ = sleep(Duration::from_millis(500)) => {}
	}

	// Event created by external process
	let file_path = dir.join("demo2.txt");
	match env::consts::OS {
		"linux" | "macos" => Command::new("touch")
			.arg(file_path.to_str().unwrap())
			.spawn()
			.unwrap(),
		"windows" => Command::new("cmd")
			.arg("/C")
			.arg("echo")
			.arg("hello")
			.arg(">")
			.arg(file_path.to_str().unwrap())
			.spawn()
			.unwrap(),
		_ => panic!("Unsupported OS"),
	};
	assert_eq!(
		rx.recv().await.unwrap(),
		Event::Create(Type::File, file_path)
	);
}

// TODO: Test that events while the worker is restarting are emitted once it is resumed

// TODO: Test SymLink and HardLink's in `basic_tests` function

// TODO: Root create/delete -> Confirm how the watcher acts when the watched directory is removed/delete. A special event should be emitted for this.
