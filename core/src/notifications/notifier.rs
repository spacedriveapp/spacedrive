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
		// TODO: Restore notifications from the DB

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

	pub async fn clear_notifications(&self) {
		self.notifications.lock().await.clear();
	}

	pub fn subscribe(&self) -> broadcast::Receiver<Notification> {
		self.chan.subscribe()
	}

	// When the frontend responds to a notification
	pub fn handle_notification_callback(&self) {
		match "todo" {
			"spacedrop" => {
				let drop_id = "todo"; // TODO: Decode from incoming data

				// TODO: Emit back out to Spacedrop system
			}
			_ => panic!("TODO: Error handling"),
		}
	}
}
