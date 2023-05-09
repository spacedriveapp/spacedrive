use std::sync::Arc;

use tokio::sync::{broadcast, Mutex};

use super::Notification;

pub struct Notifier {
	// TODO: Store to backend so they can be retrieved after restart
	notifications: Mutex<Vec<Notification>>,
	chan: broadcast::Sender<Notification>,
}

impl Notifier {
	pub fn new() -> Arc<Self> {
		Arc::new(Self {
			notifications: Mutex::new(Vec::new()),
			chan: broadcast::channel(15).0,
		})
	}

	// TODO: Library specific vs node notifications
	pub async fn emit(&self, notification: Notification) {
		self.notifications.lock().await.push(notification.clone());
		if self.chan.receiver_count() > 0 {
			self.chan.send(notification).ok();
		}
	}

	pub async fn get_notifications(&self) -> Vec<Notification> {
		self.notifications.lock().await.clone()
	}

	pub async fn clear_notifications(&mut self) {
		self.notifications.lock().await.clear();
	}

	pub fn subscribe(&self) -> broadcast::Receiver<Notification> {
		self.chan.subscribe()
	}
}
