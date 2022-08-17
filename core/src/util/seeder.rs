use crate::{
	location::indexer::{
		indexer_rules::{IndexerRule, ParametersPerKind, RuleKind},
		IndexerError,
	},
	prisma::PrismaClient,
};
use globset::Glob;
use thiserror::Error;

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
