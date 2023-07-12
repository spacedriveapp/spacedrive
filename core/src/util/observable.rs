#![allow(dead_code)]

use std::{
	collections::hash_map::DefaultHasher,
	hash::{Hash, Hasher},
	ops::{Deref, DerefMut},
};

use tokio::sync::{Notify, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// A simple JS-style observable in Rust
pub struct Observable<T> {
	t: RwLock<T>,
	notify: Notify,
}

impl<T> Observable<T>
where
	T: Hash,
{
	pub fn new(t: T) -> Self {
		Self {
			t: RwLock::new(t),
			notify: Notify::new(),
		}
	}

	pub async fn get_mut(&self) -> ObservableRef<'_, T> {
		let t = self.t.write().await;

		ObservableRef {
			start_hash: {
				let mut s = DefaultHasher::new();
				t.hash(&mut s);
				s.finish()
			},
			t,
			notify: &self.notify,
		}
	}

	pub async fn set(&self, t: T) {
		*self.get_mut().await = t;
	}

	pub async fn get(&self) -> RwLockReadGuard<'_, T> {
		self.t.read().await
	}

	/// Wait until the value changes, then return the new value
	pub async fn wait(&self) -> T
	where
		T: Clone,
	{
		self.notify.notified().await;
		self.t.read().await.clone()
	}
}

pub struct ObservableRef<'a, T>
where
	T: Hash,
{
	t: RwLockWriteGuard<'a, T>,
	notify: &'a Notify,
	start_hash: u64,
}

impl<T> Deref for ObservableRef<'_, T>
where
	T: Hash,
{
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.t
	}
}

impl<T> DerefMut for ObservableRef<'_, T>
where
	T: Hash,
{
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.t
	}
}

impl<T> Drop for ObservableRef<'_, T>
where
	T: Hash,
{
	fn drop(&mut self) {
		let mut s = DefaultHasher::new();
		self.t.hash(&mut s);

		if self.start_hash != s.finish() {
			self.notify.notify_waiters();
		}
	}
}
