use futures_util::StreamExt;
use opendal::Operator;

// TODO: Error handling
// TODO: Tracing

pub async fn ephemeral(opendal: Operator) {
	let mut lister = opendal.lister("/").await.unwrap();

	// We must not keep `entry` around as we will quickly hit the OS limit on open file descriptors
	while let Some(entry) = lister.next().await {
		let entry = entry.unwrap();

		println!("{:?}", entry);

		// entries.push(Entry {
		// 	path: entry.path(),
		// 	name: entry
		// 		.file_name()
		// 		.to_str()
		// 		.ok_or_else(|| {
		// 			(
		// 				path,
		// 				io::Error::new(ErrorKind::Other, "error non UTF-8 path"),
		// 			)
		// 		})?
		// 		.to_string(),
		// 	metadata: entry.metadata().map_err(|e| (path, e))?,
		// });
	}
}

// tokio::task::spawn_blocking(move || {
// 	let path = &path;
// 	let dir = std::fs::read_dir(path).map_err(|e| (path, e))?;
// 	let mut entries = Vec::new();
// 	for entry in dir {
// 		let entry = entry.map_err(|e| (path, e))?;

// 		// We must not keep `entry` around as we will quickly hit the OS limit on open file descriptors
// 		entries.push(Entry {
// 			path: entry.path(),
// 			name: entry
// 				.file_name()
// 				.to_str()
// 				.ok_or_else(|| {
// 					(
// 						path,
// 						io::Error::new(ErrorKind::Other, "error non UTF-8 path"),
// 					)
// 				})?
// 				.to_string(),
// 			metadata: entry.metadata().map_err(|e| (path, e))?,
// 		});
// 	}

// 	Ok(entries)
// })
// .await?
