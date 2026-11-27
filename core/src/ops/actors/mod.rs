//! # Actors
//!
//! Who made a change: a person, a device, or an agent. Every space event
//! names one, and every new grant is lent to one, so an agent's history
//! follows it across the grants it holds. Changes made through the app are
//! the person on this device; changes the watcher saw are this device, shown
//! as "changed outside Spacedrive".

use std::sync::Arc;

use chrono::Utc;
use sea_orm::{sea_query::OnConflict, ActiveValue::Set, EntityTrait};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::{
	context::CoreContext,
	infra::{
		db::entities::actor,
		query::{LibraryQuery, QueryError, QueryResult},
	},
	library::Library,
};

/// Namespace for the person actor derived from a device, so every daemon
/// that sees the device derives the same id.
const PERSON_NAMESPACE: Uuid = Uuid::from_bytes([
	0x2b, 0x8e, 0x51, 0x0c, 0x93, 0x47, 0x4d, 0x1a, 0xa6, 0x6f, 0x0e, 0x95, 0x3c, 0x71, 0xd2, 0x48,
]);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ActorKind {
	Person,
	Device,
	Agent,
}

impl ActorKind {
	pub fn as_str(self) -> &'static str {
		match self {
			Self::Person => "person",
			Self::Device => "device",
			Self::Agent => "agent",
		}
	}

	pub fn parse(kind: &str) -> Self {
		match kind {
			"person" => Self::Person,
			"device" => Self::Device,
			_ => Self::Agent,
		}
	}
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct Actor {
	pub id: Uuid,
	pub kind: ActorKind,
	pub label: String,
	pub harness: Option<String>,
}

impl From<actor::Model> for Actor {
	fn from(row: actor::Model) -> Self {
		Self {
			id: row.uuid,
			kind: ActorKind::parse(&row.kind),
			label: row.label,
			harness: row.harness,
		}
	}
}

async fn ensure(
	library: &Library,
	id: Uuid,
	kind: ActorKind,
	label: String,
	harness: Option<String>,
	created_by: Option<Uuid>,
) -> Result<Uuid, sea_orm::DbErr> {
	let result = actor::Entity::insert(actor::ActiveModel {
		uuid: Set(id),
		kind: Set(kind.as_str().to_string()),
		label: Set(label),
		harness: Set(harness),
		created_by: Set(created_by),
		created_at: Set(Utc::now()),
		..Default::default()
	})
	.on_conflict(
		OnConflict::column(actor::Column::Uuid)
			.do_nothing()
			.to_owned(),
	)
	.exec_without_returning(library.db().conn())
	.await;
	match result {
		Ok(_) | Err(sea_orm::DbErr::RecordNotInserted) => Ok(id),
		Err(error) => Err(error),
	}
}

/// This device as an actor: what the watcher attributes outside changes to.
/// Its id is the device id.
pub async fn this_device(library: &Library, context: &CoreContext) -> Result<Uuid, String> {
	let device = context
		.device_manager
		.config()
		.map_err(|e| format!("no device identity: {e}"))?;
	ensure(
		library,
		device.id,
		ActorKind::Device,
		device.name,
		None,
		None,
	)
	.await
	.map_err(|e| e.to_string())
}

/// The person using this device: what app actions are attributed to.
pub async fn this_person(library: &Library, context: &CoreContext) -> Result<Uuid, String> {
	let device = this_device(library, context).await?;
	let label = std::env::var("USER")
		.or_else(|_| std::env::var("USERNAME"))
		.unwrap_or_else(|_| "You".to_string());
	ensure(
		library,
		Uuid::new_v5(&PERSON_NAMESPACE, device.as_bytes()),
		ActorKind::Person,
		label,
		None,
		Some(device),
	)
	.await
	.map_err(|e| e.to_string())
}

/// A new agent, created by the person on this device.
pub async fn new_agent(
	library: &Library,
	context: &CoreContext,
	label: String,
	harness: Option<String>,
) -> Result<Uuid, String> {
	let person = this_person(library, context).await?;
	ensure(
		library,
		Uuid::now_v7(),
		ActorKind::Agent,
		label,
		harness,
		Some(person),
	)
	.await
	.map_err(|e| e.to_string())
}

/// Every actor in the library.
pub async fn all(library: &Library) -> Result<Vec<Actor>, sea_orm::DbErr> {
	Ok(actor::Entity::find()
		.all(library.db().conn())
		.await?
		.into_iter()
		.map(Actor::from)
		.collect())
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ActorsListInput {}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ActorsListOutput {
	pub actors: Vec<Actor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorsListQuery;

impl LibraryQuery for ActorsListQuery {
	type Input = ActorsListInput;
	type Output = ActorsListOutput;

	fn from_input(_input: Self::Input) -> QueryResult<Self> {
		Ok(Self)
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let library_id = session
			.current_library_id
			.ok_or_else(|| QueryError::Internal("No library selected".to_string()))?;
		let library = context
			.libraries()
			.await
			.get_library(library_id)
			.await
			.ok_or(QueryError::LibraryNotFound(library_id))?;
		Ok(ActorsListOutput {
			actors: all(&library).await?,
		})
	}
}

crate::register_library_query!(ActorsListQuery, "actors.list");
