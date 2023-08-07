use async_stream::stream;
use chrono::{DateTime, Utc};
use futures::future::join_all;
use rspc::{alpha::AlphaRouter, ErrorCode};
use sd_prisma::prisma::notification;
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::api::{Ctx, R};

use super::utils::library;

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

/// Represents the data of a single notification.
/// This data is used by the frontend to properly display the notification.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum NotificationData {
	PairingRequest { id: Uuid, pairing_id: u16 },
	Test,
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
							.map_err(|err| {
								rspc::Error::new(
									ErrorCode::InternalServerError,
									format!(
										"Failed to get notifications for library '{}': {}",
										library.id, err
									),
								)
							})?
							.into_iter()
							.map(|n| {
								Ok(Notification {
									id: NotificationId::Library(library.id, n.id as u32),
									data: rmp_serde::from_slice(&n.data).map_err(|err| {
										rspc::Error::new(
											ErrorCode::InternalServerError,
											format!(
												"Failed to get notifications for library '{}': {}",
												library.id, err
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
							.map_err(|err| {
								rspc::Error::new(ErrorCode::InternalServerError, err.to_string())
							})?;
					}
					NotificationId::Node(id) => {
						node.config
							.write(|mut cfg| {
								cfg.notifications
									.retain(|n| n.id != NotificationId::Node(id));
							})
							.await
							.map_err(|err| {
								rspc::Error::new(ErrorCode::InternalServerError, err.to_string())
							})?;
					}
				}

				Ok(())
			})
		})
		.procedure("dismissAll", {
			R.query(|node, _: ()| async move {
				node.config
					.write(|mut cfg| {
						cfg.notifications = vec![];
					})
					.await
					.map_err(|err| {
						rspc::Error::new(ErrorCode::InternalServerError, err.to_string())
					})?;

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
		.procedure("test", {
			R.mutation(|node, _: ()| async move {
				node.emit_notification(NotificationData::Test, None).await;
			})
		})
		.procedure("testLibrary", {
			R.with2(library())
				.mutation(|(_, library), _: ()| async move {
					library
						.emit_notification(NotificationData::Test, None)
						.await;
				})
		})
}
