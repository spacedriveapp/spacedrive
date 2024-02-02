use sd_task_system::TaskSystem;

use std::time::Duration;

use rand::Rng;
use tempfile::tempdir;
use tokio::time::sleep;
use tracing_test::traced_test;

mod actors;
// mod jobs;

use actors::SampleActor;

#[tokio::test]
#[traced_test]
async fn test_actor() {
	let data_dir = tempdir().unwrap();

	let system = TaskSystem::new().await;

	let actor =
		SampleActor::new(data_dir.path(), "test".to_string(), system.get_dispatcher()).await;

	let mut rng = rand::thread_rng();

	for _ in 0..1000 {
		if rng.gen_bool(0.1) {
			actor.process_with_priority(Duration::from_millis(50)).await;
		} else {
			actor.process(Duration::from_millis(100)).await;
		}
	}

	sleep(Duration::from_secs(50)).await;
}
