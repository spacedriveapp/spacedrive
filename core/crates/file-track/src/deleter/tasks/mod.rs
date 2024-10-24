mod move_to_trash;
mod remove;

use std::{
	marker::PhantomData,
	sync::{
		atomic::{AtomicU64, Ordering},
		Arc,
	},
};

pub use move_to_trash::MoveToTrashBehavior;
pub use remove::RemoveBehavior;
use sd_task_system::{check_interruption, ExecStatus, Interrupter, SerializableTask, Task, TaskId};
use serde::{Deserialize, Serialize};

use crate::deleter::DeleteBehavior;
use crate::deleter::FileData;

pub type MoveToTrash = RemoveTask<MoveToTrashBehavior>;
pub type Remove = RemoveTask<RemoveBehavior>;

pub struct RemoveTask<B> {
	id: TaskId,
	files: Vec<FileData>,
	counter: Arc<AtomicU64>,
	behavior: PhantomData<fn(B) -> B>,
}

impl<B: DeleteBehavior> RemoveTask<B> {
	pub fn new(files: Vec<FileData>, counter: Arc<AtomicU64>) -> Self {
		Self {
			id: TaskId::new_v4(),
			files,
			counter,
			behavior: PhantomData,
		}
	}
}

#[async_trait::async_trait]
impl<B: DeleteBehavior + Send + 'static> Task<super::Error> for RemoveTask<B> {
	fn id(&self) -> TaskId {
		self.id
	}

	async fn run(&mut self, interrupter: &Interrupter) -> Result<ExecStatus, super::Error> {
		tracing::debug!(id=%self.id, "running remove task");

		check_interruption!(interrupter);

		let size = self.files.len();

		// TODO(matheus-consoli): error handling
		let x = B::delete_all(self.files.clone(), Some(interrupter)).await;

		if let Ok(res) = x {
			if let ExecStatus::Done(_) = &res {
				self.counter.fetch_add(size as _, Ordering::AcqRel);
			}
			Ok(res)
		} else {
			Err(super::Error::Deleter("wtf".into()))
		}
	}
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RemoveTaskState {
	id: TaskId,
	files: Vec<FileData>,
	counter: Arc<AtomicU64>,
	kind: SavedTaskKind,
}

#[derive(Debug, Serialize, Deserialize)]
enum SavedTaskKind {
	Remove,
	MoveToTrash,
}

impl From<PhantomData<fn(RemoveBehavior) -> RemoveBehavior>> for SavedTaskKind {
	fn from(_: PhantomData<fn(RemoveBehavior) -> RemoveBehavior>) -> Self {
		SavedTaskKind::Remove
	}
}

impl From<PhantomData<fn(MoveToTrashBehavior) -> MoveToTrashBehavior>> for SavedTaskKind {
	fn from(_: PhantomData<fn(MoveToTrashBehavior) -> MoveToTrashBehavior>) -> Self {
		SavedTaskKind::MoveToTrash
	}
}

impl TryFrom<SavedTaskKind> for PhantomData<fn(RemoveBehavior) -> RemoveBehavior> {
	type Error = ();

	fn try_from(value: SavedTaskKind) -> Result<Self, Self::Error> {
		match value {
			SavedTaskKind::Remove => Ok(PhantomData),
			SavedTaskKind::MoveToTrash => Err(()),
		}
	}
}

impl TryFrom<SavedTaskKind> for PhantomData<fn(MoveToTrashBehavior) -> MoveToTrashBehavior> {
	type Error = ();

	fn try_from(value: SavedTaskKind) -> Result<Self, Self::Error> {
		match value {
			SavedTaskKind::MoveToTrash => Ok(PhantomData),
			SavedTaskKind::Remove => Err(()),
		}
	}
}

impl<B> SerializableTask<super::Error> for RemoveTask<B>
where
	B: DeleteBehavior + Send + 'static,
	SavedTaskKind: From<PhantomData<fn(B) -> B>>,
	PhantomData<fn(B) -> B>: TryFrom<SavedTaskKind>,
{
	type SerializeError = rmp_serde::encode::Error;
	type DeserializeError = rmp_serde::decode::Error;
	type DeserializeCtx = ();

	async fn serialize(self) -> Result<Vec<u8>, Self::SerializeError> {
		let Self {
			id,
			files,
			counter,
			behavior,
		} = self;

		let state = RemoveTaskState {
			id,
			files,
			counter,
			kind: behavior.into(),
		};

		rmp_serde::to_vec_named(&state)
	}

	async fn deserialize(
		data: &[u8],
		_: Self::DeserializeCtx,
	) -> Result<Self, Self::DeserializeError> {
		rmp_serde::from_slice(data).map(|state: RemoveTaskState| Self {
			id: state.id,
			files: state.files,
			counter: state.counter,
			behavior: state.kind.try_into().map_err(|_| ()).unwrap(),
		})
	}
}
