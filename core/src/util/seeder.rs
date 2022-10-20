use std::{sync::Arc, str::FromStr};

use crate::{
	location::indexer::{
		rules::{IndexerRule, ParametersPerKind, RuleKind},
		IndexerError,
	},
	prisma::PrismaClient,
};
use globset::Glob;
use sd_crypto::{keys::{keymanager::{KeyManager, StoredKey}, hashing::HashingAlgorithm}, crypto::stream::Algorithm, primitives::to_array};
use thiserror::Error;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum SeederError {
	#[error("Failed to run indexer rules seeder: {0}")]
	IndexerRules(#[from] IndexerError),
	#[error("An error occurred with the database while applying migrations: {0}")]
	DatabaseError(#[from] prisma_client_rust::QueryError),
}

pub async fn indexer_rules_seeder(client: &PrismaClient) -> Result<(), SeederError> {
	if client.indexer_rule().count(vec![]).exec().await? == 0 {
		for rule in [
			IndexerRule::new(
				RuleKind::RejectFilesByGlob,
				"Reject Hidden Files".to_string(),
				ParametersPerKind::RejectFilesByGlob(
					Glob::new("**/.*").map_err(IndexerError::GlobBuilderError)?,
				),
			),
			IndexerRule::new(
				RuleKind::AcceptIfChildrenDirectoriesArePresent,
				"Git Repositories".into(),
				ParametersPerKind::AcceptIfChildrenDirectoriesArePresent(
					[".git".to_string()].into_iter().collect(),
				),
			),
			IndexerRule::new(
				RuleKind::AcceptFilesByGlob,
				"Only Images".to_string(),
				ParametersPerKind::AcceptFilesByGlob(
					Glob::new("*.{jpg,png,jpeg,gif,webp}")
						.map_err(IndexerError::GlobBuilderError)?,
				),
			),
		] {
			rule.save(client).await?;
		}
	}

	Ok(())
}

pub async fn keystore_seeder(client: &PrismaClient, key_manager: Arc<Mutex<KeyManager>>) -> Result<(), SeederError> {
		// retrieve all stored keys from the DB
		let db_stored_keys = client.key().find_many(vec![]).exec().await?;

		let mut default = Uuid::default();

		// collect and serialize the stored keys
		let stored_keys: Vec<StoredKey> = db_stored_keys.iter().map(|d| {
			let d = d.clone();
			let uuid = uuid::Uuid::from_str(&d.uuid).unwrap();

			if d.default {
				default = uuid.clone();
			}

			StoredKey {
				uuid,
				salt: to_array(d.salt).unwrap(),
				algorithm: Algorithm::deserialize(to_array(d.algorithm).unwrap()).unwrap(),
				content_salt: to_array(d.content_salt).unwrap(),
				master_key: to_array(d.master_key).unwrap(),
				master_key_nonce: d.master_key_nonce,
				key_nonce: d.key_nonce,
				key: d.key,
				hashing_algorithm: HashingAlgorithm::deserialize(to_array(d.hashing_algorithm).unwrap()).unwrap(),
			}
		}).collect();

		// insert all keys from the DB into the keymanager's keystore
		key_manager.lock().await.populate_keystore(stored_keys).unwrap();

		// if any key had an associated default tag
		if !default.is_nil() {
			key_manager.lock().await.set_default(default).unwrap();
		}

	Ok(())
}