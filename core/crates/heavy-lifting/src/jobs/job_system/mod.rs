use crate::Error;

use sd_task_system::TaskDispatcher;

pub(crate) mod job;
pub(crate) mod report;

pub struct JobSystem {
	dispatcher: TaskDispatcher<Error>,
}

enum Command {
	Pause,
	Resume,
	Cancel,
}
