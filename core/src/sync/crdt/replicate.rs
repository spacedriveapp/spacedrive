use crate::sync::engine::SyncContext;
use serde::{Deserialize, Serialize};

#[async_trait::async_trait]
pub trait Replicate {
	type Create: Clone;

	async fn create(data: Self::Create, ctx: SyncContext)
	where
		Self: Sized;

	async fn delete(ctx: SyncContext)
	where
		Self: Sized;
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ReplicateMethod<T: Replicate + Clone> {
	Create(T::Create),
}

impl<T: Replicate + Clone> ReplicateMethod<T> {
	pub fn apply(self, ctx: SyncContext) {
		match self {
			Self::Create(data) => T::create(data, ctx),
		};
	}
}
