use serde::Serialize;
use specta::Type;

#[derive(Serialize, Type, Default, Clone, Copy)]
pub enum NotificationLevel {
	#[default]
	Alert,
	Warning,
	Info,
	Success,
	Error,
}

#[derive(Serialize, Type, Default, Clone, Copy)]
pub enum NotificationStyle {
	// Purely informational and dismissable
	#[default]
	Dismiss,
	// Accept and Deny option
	AcceptDeny,
	// Accept and Cancel option
	AcceptCancel,
}

#[derive(Serialize, Type, Clone)]
pub struct Notification {
	title: String,
	level: NotificationLevel,
	style: NotificationStyle,
	// icon: // TODO
	body: Option<String>,
}

impl Notification {
	pub fn new(title: String) -> Self {
		Self {
			title,
			level: Default::default(),
			style: Default::default(),
			body: None,
		}
	}

	pub fn level(self, level: NotificationLevel) -> Self {
		Self { level, ..self }
	}

	pub fn style(self, style: NotificationStyle) -> Self {
		Self { style, ..self }
	}

	pub fn body(self, body: String) -> Self {
		Self {
			body: Some(body),
			..self
		}
	}
}
