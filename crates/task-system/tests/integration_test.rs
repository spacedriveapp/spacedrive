use sd_task_system::{TaskStatus, TaskSystem};
use tracing::{info, trace};

use std::time::Duration;

use rand::Rng;
use tempfile::tempdir;
use tracing_test::traced_test;

mod actors;
// mod jobs;
mod tasks;

use actors::SampleActor;
use tasks::NeverTask;

use crate::tasks::ReadyTask;

#[tokio::test]
#[traced_test]
async fn test_actor() {
	let data_dir = tempdir().unwrap();

	let system = TaskSystem::new().await;

	let (actor, mut actor_idle_rx) =
		SampleActor::new(data_dir.path(), "test".to_string(), system.get_dispatcher()).await;

	let mut rng = rand::thread_rng();

	for i in 0..=500 {
		if rng.gen_bool(0.1) {
			info!("dispatching priority task {i}");
			actor
				.process_with_priority(Duration::from_millis(rng.gen_range(50..150)))
				.await;
		} else {
			info!("dispatching task {i}");
			actor
				.process(Duration::from_millis(rng.gen_range(200..1000)))
				.await;
		}
	}

	info!("all tasks dispatched, now we wait a bit...");

	actor_idle_rx.recv().await.unwrap();

	system.shutdown().await;

	info!("done");
}

#[tokio::test]
#[traced_test]
async fn shutdown_test() {
	let system = TaskSystem::new().await;

	let handle = system.dispatch(NeverTask::default()).await;

	system.shutdown().await;

	assert!(matches!(handle.await, Ok(TaskStatus::Shutdown(_))));
}

#[tokio::test]
#[traced_test]
async fn cancel_test() {
	let system = TaskSystem::new().await;

	let handle = system.dispatch(NeverTask::default()).await;

	info!("issuing cancel");
	handle.cancel().await.unwrap();

	assert!(matches!(handle.await, Ok(TaskStatus::Canceled)));

	system.shutdown().await;
}

#[tokio::test]
#[traced_test]
async fn done_test() {
	let system = TaskSystem::new().await;

	let handle = system.dispatch(ReadyTask::default()).await;

	assert!(matches!(handle.await, Ok(TaskStatus::Done)));

	system.shutdown().await;
}

#[tokio::test]
#[traced_test]
async fn abort_test() {
	let system = TaskSystem::new().await;

	let handle = system.dispatch(NeverTask::default()).await;

	handle.force_abortion().await.unwrap();

	let res = handle.await;

	trace!("abort test result: {res:#?}");

	assert!(matches!(res, Ok(TaskStatus::ForcedAbortion)));

	system.shutdown().await;
}
