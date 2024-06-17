use sd_prisma::prisma::notification;

use crate::api::{Ctx, R};
use async_stream::stream;
use chrono::{DateTime, Utc};
use futures::future::join_all;
use rspc::{alpha::AlphaRouter, ErrorCode};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

/// Represents a single notification.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Notification {
	#[serde(flatten)]
	pub id: NotificationId,
	pub data: NotificationData,
	pub read: bool,
	pub expires: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(tag = "type", content = "id", rename_all = "camelCase")]
pub enum NotificationId {
	Library(Uuid, u32),
	Node(u32),
}
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum NotificationKind {
	Info,
	Success,
	Error,
	Warning,
}

/// Represents the data of a single notification.
/// This data is used by the frontend to properly display the notification.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct NotificationData {
	pub title: String,
	pub content: String,
	pub kind: NotificationKind,
}

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("get", {
			R.query(|node, _: ()| async move {
				let mut notifications = node.config.get().await.notifications;
				for lib_notifications in join_all(node.libraries.get_all().await.into_iter().map(
					|library| async move {
						library
							.db
							.notification()
							.find_many(vec![])
							.exec()
							.await
							.map_err(|e| {
								rspc::Error::new(
									ErrorCode::InternalServerError,
									format!(
										"Failed to get notifications for library '{}': {}",
										library.id, e
									),
								)
							})?
							.into_iter()
							.map(|n| {
								Ok(Notification {
									id: NotificationId::Library(library.id, n.id as u32),
									data: rmp_serde::from_slice(&n.data).map_err(|e| {
										rspc::Error::new(
											ErrorCode::InternalServerError,
											format!(
												"Failed to get notifications for library '{}': {}",
												library.id, e
											),
										)
									})?,
									read: false,
									expires: n.expires_at.map(Into::into),
								})
							})
							.collect::<Result<Vec<Notification>, rspc::Error>>()
					},
				))
				.await
				{
					notifications.extend(lib_notifications?);
				}

				Ok(notifications)
			})
		})
		.procedure("dismiss", {
			R.query(|node, id: NotificationId| async move {
				match id {
					NotificationId::Library(library_id, id) => {
						node.libraries
							.get_library(&library_id)
							.await
							.ok_or_else(|| {
								rspc::Error::new(ErrorCode::NotFound, "Library not found".into())
							})?
							.db
							.notification()
							.delete_many(vec![notification::id::equals(id as i32)])
							.exec()
							.await
							.map_err(|e| {
								rspc::Error::new(ErrorCode::InternalServerError, e.to_string())
							})?;
					}
					NotificationId::Node(id) => {
						node.config
							.write(|cfg| {
								cfg.notifications
									.retain(|n| n.id != NotificationId::Node(id));
							})
							.await
							.map_err(|e| {
								rspc::Error::new(ErrorCode::InternalServerError, e.to_string())
							})?;
					}
				}

				Ok(())
			})
		})
		.procedure("dismissAll", {
			R.query(|node, _: ()| async move {
				node.config
					.write(|cfg| {
						cfg.notifications = vec![];
					})
					.await
					.map_err(|e| rspc::Error::new(ErrorCode::InternalServerError, e.to_string()))?;

				join_all(
					node.libraries
						.get_all()
						.await
						.into_iter()
						.map(|library| async move {
							library.db.notification().delete_many(vec![]).exec().await
						}),
				)
				.await
				.into_iter()
				.collect::<Result<Vec<_>, _>>()?;

				Ok(())
			})
		})
		.procedure("listen", {
			R.subscription(|node, _: ()| async move {
				let mut sub = node.notifications.subscribe();

				stream! {
					while let Ok(notification) = sub.recv().await {
						yield notification;
					}
				}
			})
		})
}
