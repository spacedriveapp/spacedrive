use std::pin::pin;

use async_stream::stream;
use futures::{Stream, StreamExt};
use serde::Serialize;
use specta::Type;

#[derive(Serialize, Type)]
#[serde(untagged)]
pub enum Output<T> {
	Data(T),
	Complete { __stream_complete: () },
}

pub fn unsafe_streamed_query<S: Stream>(stream: S) -> impl Stream<Item = Output<S::Item>> {
	stream! {
		let mut stream = pin!(stream);
		while let Some(v) = stream.next().await {
			yield Output::Data(v);
		}

		yield Output::Complete { __stream_complete: () };
	}
}
