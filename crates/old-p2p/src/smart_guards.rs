use std::{
	ops::{Deref, DerefMut},
	sync::RwLockWriteGuard,
};

use crate::P2P;

type SaveFn<T> = fn(&P2P, /* before */ T, /* after */ &T);

/// A special guard for `RwLockWriteGuard` that will call a `save` function when it's dropped.
/// This allows changes to the value to automatically trigger `HookEvents` to be emitted.
#[derive(Debug)]
pub struct SmartWriteGuard<'a, T> {
	p2p: &'a P2P,
	lock: RwLockWriteGuard<'a, T>,
	before: Option<T>,
	save: SaveFn<T>,
}

impl<'a, T: Clone> SmartWriteGuard<'a, T> {
	pub(crate) fn new(p2p: &'a P2P, lock: RwLockWriteGuard<'a, T>, save: SaveFn<T>) -> Self {
		Self {
			p2p,
			before: Some(lock.clone()),
			lock,
			save,
		}
	}
}

impl<T> Deref for SmartWriteGuard<'_, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.lock
	}
}

impl<T> DerefMut for SmartWriteGuard<'_, T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.lock
	}
}

impl<T> Drop for SmartWriteGuard<'_, T> {
	fn drop(&mut self) {
		(self.save)(
			self.p2p,
			self.before
				.take()
				.expect("'SmartWriteGuard::drop' called more than once!"),
			&self.lock,
		);
	}
}
