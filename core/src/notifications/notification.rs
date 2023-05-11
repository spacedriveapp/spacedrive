use chrono::{DateTime, FixedOffset, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::prisma::notification;

#[derive(Debug, Serialize, Deserialize, Type, Default, Clone, Copy)]
pub enum NotificationLevel {
	#[default]
	Alert,
	Warning,
	Info,
	Success,
	Error,
}

#[derive(Debug, Serialize, Deserialize, Type, Default, Clone, Copy)]
pub enum NotificationStyle {
	// Purely informational and dismissable
	#[default]
	Dismiss,
	// Accept and Deny option
	AcceptDeny,
	// Accept and Cancel option
	AcceptCancel,
}

#[derive(Debug, Serialize, Deserialize, Type, Clone)]
pub struct Notification {
	id: Uuid,
	title: String,
	level: NotificationLevel,
	style: NotificationStyle,
	// Has the user seem it?
	read: bool,
	// icon: // TODO
	body: Option<String>,
	created_at: DateTime<FixedOffset>,
}

impl Notification {
	pub fn new(title: String) -> Self {
		Self {
			id: Uuid::new_v4(),
			title,
			level: Default::default(),
			style: Default::default(),
			read: false,
			body: None,
			// TODO: This is without a dobt gonna cause an issue but it's also being used in `api/filter.rs`
			created_at: Utc::now().with_timezone(&FixedOffset::east_opt(0).unwrap()),
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

impl From<notification::Data> for Notification {
	fn from(value: notification::Data) -> Self {
		Self {
			id: Uuid::from_slice(&*value.id).unwrap(),
			title: value.title,
			level: match &*value.level {
				"alert" => NotificationLevel::Alert,
				"warning" => NotificationLevel::Warning,
				"info" => NotificationLevel::Info,
				"success" => NotificationLevel::Success,
				"error" => NotificationLevel::Error,
				_ => panic!(),
			},
			style: match &*value.style {
				"dismiss" => NotificationStyle::Dismiss,
				"accept_deny" => NotificationStyle::AcceptDeny,
				"accept_cancel" => NotificationStyle::AcceptCancel,
				_ => panic!(),
			},
			read: value.read,
			body: value.body,
			created_at: value.created_at,
		}
	}
}

impl Into<notification::Data> for Notification {
	fn into(self) -> notification::Data {
		notification::Data {
			id: self.id.as_bytes().to_vec(),
			title: self.title,
			level: match self.level {
				NotificationLevel::Alert => "alert".to_string(),
				NotificationLevel::Warning => "warning".to_string(),
				NotificationLevel::Info => "info".to_string(),
				NotificationLevel::Success => "success".to_string(),
				NotificationLevel::Error => "error".to_string(),
			},
			style: match self.style {
				NotificationStyle::Dismiss => "dismiss".to_string(),
				NotificationStyle::AcceptDeny => "accept_deny".to_string(),
				NotificationStyle::AcceptCancel => "accept_cancel".to_string(),
			},
			read: self.read,
			body: self.body,
			created_at: self.created_at,
		}
	}
}
