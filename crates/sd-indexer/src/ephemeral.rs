use std::path::PathBuf;

use chrono::{DateTime, Utc};
use futures_util::{Stream, StreamExt};
use opendal::Operator;
use serde::Serialize;
use specta::Type;

use crate::stream::TaskStream;

// TODO: Error handling
// TODO: Tracing

// TODO: Sorting -> Probs frontend now
// TODO: IndexerRule's
// TODO: Thumbnailer

// TODO: Do in within the app but if it's a location lookup thing

pub async fn ephemeral(opendal: Operator, path: PathBuf) -> impl Stream<Item = NonIndexedPathItem> {
	let path = path.to_str().unwrap().to_string();
	let mut lister = opendal.lister(&path).await.unwrap();

	TaskStream::new(move |tx| async move {
		while let Some(entry) = lister.next().await {
			let entry = entry.unwrap();
			let path = PathBuf::from(entry.path());

			tx.send(NonIndexedPathItem {
				path: entry.path().to_string(),
				name: path
					.file_name()
					.unwrap()
					.to_str()
					.unwrap()
					// .ok_or_else(|| {
					// 	(
					// 		path,
					// 		io::Error::new(ErrorKind::Other, "error non UTF-8 path"),
					// 	)
					// })?
					.to_string(),
				extension: path
					.extension()
					.and_then(|s| s.to_str().map(str::to_string))
					.unwrap_or_default(),
				kind: 0, // TODO
				is_dir: entry.metadata().is_dir(),
				date_created: Default::default(),                 // TODO
				date_modified: Default::default(),                // TODO
				size_in_bytes_bytes: 0u64.to_be_bytes().to_vec(), // TODO
				hidden: false,                                    // TODO
			})
			.await
			.unwrap(); // TODO: Abort on exit cause the stream has been dropped
		}
	})
}

#[derive(Serialize, Type, Debug)]
pub struct NonIndexedPathItem {
	pub path: String,
	pub name: String,
	pub extension: String,
	pub kind: i32,
	pub is_dir: bool,
	pub date_created: DateTime<Utc>,
	pub date_modified: DateTime<Utc>,
	pub size_in_bytes_bytes: Vec<u8>,
	pub hidden: bool,
}
