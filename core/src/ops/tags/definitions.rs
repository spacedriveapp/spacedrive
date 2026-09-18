//! Definitions across their homes.
//!
//! A definition lives in every source store that applies it, and in the
//! library's `tag_staging` table while it is applied nowhere. Reads union
//! both, deduplicated by uuid with the latest stamp winning, so a rename that
//! reached one store is what every listing shows.

use std::collections::HashMap;
use std::sync::Arc;

use sd_store::TagDefinition;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

use crate::infra::db::entities::tag_staging;
use crate::library::Library;
use crate::ops::indexing::ephemeral::EphemeralIndexCache;

fn from_staging(model: tag_staging::Model) -> TagDefinition {
	TagDefinition {
		uuid: model.uuid,
		slug_id: model.slug_id,
		path: model.path,
		color: model.color,
		icon: model.icon,
		updated_hlc: model.updated_hlc,
		origin_device: model.origin_device,
	}
}

fn keep_later(map: &mut HashMap<Uuid, TagDefinition>, definition: TagDefinition) {
	match map.get(&definition.uuid) {
		Some(existing) if existing.updated_hlc >= definition.updated_hlc => {}
		_ => {
			map.insert(definition.uuid, definition);
		}
	}
}

/// Every definition this machine can name, staged ones included, ordered by
/// path. A store that fails to answer is skipped with a warning: a listing
/// is best effort, and the store's own open path reports its failure.
pub async fn all(library: &Library, cache: &EphemeralIndexCache) -> Vec<TagDefinition> {
	let mut by_uuid: HashMap<Uuid, TagDefinition> = HashMap::new();

	match tag_staging::Entity::find().all(library.db().conn()).await {
		Ok(models) => {
			for model in models {
				keep_later(&mut by_uuid, from_staging(model));
			}
		}
		Err(error) => tracing::warn!(%error, "staged tag definitions unavailable"),
	}

	for store in cache.stores().await {
		match store.db().tag_definitions().await {
			Ok(definitions) => {
				for definition in definitions {
					keep_later(&mut by_uuid, definition);
				}
			}
			Err(error) => {
				tracing::warn!(source = %store.id(), %error, "tag definitions unavailable")
			}
		}
	}

	let mut definitions: Vec<TagDefinition> = by_uuid.into_values().collect();
	definitions.sort_by(|a, b| a.path.cmp(&b.path));
	definitions
}

/// The named definitions, and the ids nothing on this machine can name.
pub async fn find(
	library: &Library,
	cache: &EphemeralIndexCache,
	ids: &[Uuid],
) -> (Vec<TagDefinition>, Vec<Uuid>) {
	let known: HashMap<Uuid, TagDefinition> = all(library, cache)
		.await
		.into_iter()
		.map(|definition| (definition.uuid, definition))
		.collect();

	let mut found = Vec::with_capacity(ids.len());
	let mut missing = Vec::new();
	for id in ids {
		match known.get(id) {
			Some(definition) => found.push(definition.clone()),
			None => missing.push(*id),
		}
	}
	(found, missing)
}

/// One definition by uuid.
pub async fn find_one(
	library: &Library,
	cache: &EphemeralIndexCache,
	id: Uuid,
) -> Option<TagDefinition> {
	let (found, _) = find(library, cache, &[id]).await;
	found.into_iter().next()
}

/// An existing definition for this slug, wherever it lives. What makes
/// `tags.create` idempotent: the same path names the same tag.
pub async fn find_by_slug(
	library: &Library,
	cache: &EphemeralIndexCache,
	slug: Uuid,
) -> Option<TagDefinition> {
	all(library, cache)
		.await
		.into_iter()
		.find(|definition| definition.slug_id == slug)
}

/// Hold a definition that is applied nowhere yet.
pub async fn stage(library: &Library, definition: &TagDefinition) -> Result<(), sea_orm::DbErr> {
	tag_staging::ActiveModel {
		uuid: Set(definition.uuid),
		slug_id: Set(definition.slug_id),
		path: Set(definition.path.clone()),
		color: Set(definition.color.clone()),
		icon: Set(definition.icon.clone()),
		updated_hlc: Set(definition.updated_hlc.clone()),
		origin_device: Set(definition.origin_device),
		..Default::default()
	}
	.insert(library.db().conn())
	.await?;
	Ok(())
}

/// Adoption: these definitions now live in a store, so staging forgets them.
pub async fn unstage(library: &Library, ids: &[Uuid]) -> Result<u64, sea_orm::DbErr> {
	if ids.is_empty() {
		return Ok(0);
	}
	let result = tag_staging::Entity::delete_many()
		.filter(tag_staging::Column::Uuid.is_in(ids.to_vec()))
		.exec(library.db().conn())
		.await?;
	Ok(result.rows_affected)
}

/// The stores that currently carry a definition, for operations that must
/// touch every home of a tag.
pub async fn stores_carrying(
	cache: &EphemeralIndexCache,
	id: Uuid,
) -> Vec<Arc<crate::ops::indexing::ephemeral::store::SourceStore>> {
	let mut carrying = Vec::new();
	for store in cache.stores().await {
		match store.db().tag_definitions().await {
			Ok(definitions) if definitions.iter().any(|d| d.uuid == id) => carrying.push(store),
			Ok(_) => {}
			Err(error) => {
				tracing::warn!(source = %store.id(), %error, "tag definitions unavailable")
			}
		}
	}
	carrying
}
