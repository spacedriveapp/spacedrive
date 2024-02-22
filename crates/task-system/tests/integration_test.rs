use sd_task_system::{TaskOutput, TaskStatus, TaskSystem};

use std::time::Duration;

use rand::Rng;
use tempfile::tempdir;
use tracing::info;
use tracing_test::traced_test;

mod common;

use common::{
	actors::SampleActor,
	tasks::{BogusTask, BrokenTask, NeverTask, PauseOnceTask, ReadyTask, SampleError},
};

use crate::common::jobs::SampleJob;

#[tokio::test]
#[traced_test]
async fn test_actor() {
	let data_dir = tempdir().unwrap();

	let system = TaskSystem::new().await;

	let (actor, mut actor_idle_rx) =
		SampleActor::new(data_dir.path(), "test".to_string(), system.get_dispatcher()).await;

	let mut rng = rand::thread_rng();

	for i in 0..=250 {
		if rng.gen_bool(0.1) {
			info!("dispatching priority task {i}");
			actor
				.process_with_priority(Duration::from_millis(rng.gen_range(50..150)))
				.await;
		} else {
			info!("dispatching task {i}");
			actor
				.process(Duration::from_millis(rng.gen_range(200..500)))
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

	assert!(matches!(
		handle.await,
		Ok(TaskStatus::Done(TaskOutput::Empty))
	));

	system.shutdown().await;
}

#[tokio::test]
#[traced_test]
async fn abort_test() {
	let system = TaskSystem::new().await;

	let (task, began_rx) = BrokenTask::new();

	let handle = system.dispatch(task).await;

	began_rx.await.unwrap();

	handle.force_abortion().await.unwrap();

	assert!(matches!(handle.await, Ok(TaskStatus::ForcedAbortion)));

	system.shutdown().await;
}

#[tokio::test]
#[traced_test]
async fn error_test() {
	let system = TaskSystem::new().await;

	let handle = system.dispatch(BogusTask::default()).await;

	assert!(matches!(
		handle.await,
		Ok(TaskStatus::Error(SampleError::SampleError))
	));

	system.shutdown().await;
}

#[tokio::test]
#[traced_test]
async fn pause_test() {
	let system = TaskSystem::new().await;

	let (task, began_rx) = PauseOnceTask::new();

	let handle = system.dispatch(task).await;

	info!("Task dispatched, now we wait for it to begin...");

	began_rx.await.unwrap();

	handle.pause().await.unwrap();

	info!("Paused task, now we resume it...");

	handle.resume().await.unwrap();

	info!("Resumed task, now we wait for it to complete...");

	assert!(matches!(
		handle.await,
		Ok(TaskStatus::Done(TaskOutput::Empty))
	));

	system.shutdown().await;
}

#[tokio::test]
#[traced_test]
async fn jobs_test() {
	let system = TaskSystem::new().await;

	let task_dispatcher = system.get_dispatcher();

	let job = SampleJob::new(256, task_dispatcher.clone());

	job.run().await.unwrap();

	system.shutdown().await;
}
