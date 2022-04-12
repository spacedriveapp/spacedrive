use crate::sync::engine::SyncContext;
use serde::{Deserialize, Serialize};

#[async_trait::async_trait]
pub trait PropertyOperation {
  type Create: Clone;
  type Update: Clone;

  async fn create(data: Self::Create, ctx: SyncContext)
  where
    Self: Sized;

  async fn update(data: Self::Update, ctx: SyncContext)
  where
    Self: Sized;

  async fn delete(ctx: SyncContext)
  where
    Self: Sized;
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PoMethod<T: PropertyOperation + Clone> {
  Create(T::Create),
  Update(T::Update),
}

impl<T: PropertyOperation + Clone> PoMethod<T> {
  pub fn apply(self, ctx: SyncContext) {
    match self {
      Self::Create(data) => T::create(data, ctx),
      Self::Update(data) => T::update(data, ctx),
    };
  }
}
