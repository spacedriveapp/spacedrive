use std::path::PathBuf;

use notify2::Watcher;

#[tokio::main]
async fn main() {
	println!("Monitoring: './demo'");
	let (mut rx, _watcher) = Watcher::new(
		vec![
			PathBuf::from(r"./demo"),
			// PathBuf::from(r"./demo2"),
			// PathBuf::from(r"/Volumes/Untitled"),
		],
		true,
	)
	.await;

	// tokio::spawn(async move {
	//     loop {
	//         sleep(Duration::from_secs(5)).await;
	//         println!("Action 1");
	//         // drop(watcher);
	//         watcher.add_paths(vec![PathBuf::from(r"./demo")]).await;
	//         println!("Action 11");
	//         sleep(Duration::from_secs(5)).await;
	//         println!("Action 2");
	//         watcher.remove_paths(vec![PathBuf::from(r"./demo")]).await;
	//         println!("Action 22");
	//     }
	// });

	while let Some(event) = rx.recv().await {
		println!("{:?}", event);
	}
}
