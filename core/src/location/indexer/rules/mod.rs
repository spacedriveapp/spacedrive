use crate::library::Library;

use sd_indexer::rules::{IndexerRuleError, RuleKind, RulePerKind};
use sd_prisma::prisma::indexer_rule;

use chrono::Utc;
use serde::Deserialize;
use specta::Type;
use tracing::debug;
use uuid::Uuid;

pub mod seed;

/// `IndexerRuleCreateArgs` is the argument received from the client using rspc to create a new indexer rule.
/// Note that `rules` field is a vector of tuples of `RuleKind` and `parameters`.
///
/// In case of  `RuleKind::AcceptFilesByGlob` or `RuleKind::RejectFilesByGlob`, it will be a
/// vector of strings containing a glob patterns.
///
/// In case of `RuleKind::AcceptIfChildrenDirectoriesArePresent` or `RuleKind::RejectIfChildrenDirectoriesArePresent` the
/// `parameters` field must be a vector of strings containing the names of the directories.
#[derive(Type, Deserialize)]
pub struct IndexerRuleCreateArgs {
	pub name: String,
	pub dry_run: bool,
	pub rules: Vec<(RuleKind, Vec<String>)>,
}

impl IndexerRuleCreateArgs {
	pub async fn create(
		self,
		library: &Library,
	) -> Result<Option<indexer_rule::Data>, IndexerRuleError> {
		debug!(
			"{} a new indexer rule (name = {}, params = {:?})",
			if self.dry_run {
				"Dry run: Would create"
			} else {
				"Trying to create"
			},
			self.name,
			self.rules
		);

		let rules_data = rmp_serde::to_vec_named(
			&self
				.rules
				.into_iter()
				.map(|(kind, parameters)| match kind {
					RuleKind::AcceptFilesByGlob => {
						RulePerKind::new_accept_files_by_globs_str(parameters)
					}
					RuleKind::RejectFilesByGlob => {
						RulePerKind::new_reject_files_by_globs_str(parameters)
					}
					RuleKind::AcceptIfChildrenDirectoriesArePresent => {
						Ok(RulePerKind::AcceptIfChildrenDirectoriesArePresent(
							parameters.into_iter().collect(),
						))
					}
					RuleKind::RejectIfChildrenDirectoriesArePresent => {
						Ok(RulePerKind::RejectIfChildrenDirectoriesArePresent(
							parameters.into_iter().collect(),
						))
					}
				})
				.collect::<Result<Vec<_>, _>>()?,
		)?;

		if self.dry_run {
			return Ok(None);
		}

		let date_created = Utc::now();

		use indexer_rule::*;

		Ok(Some(
			library
				.db
				.indexer_rule()
				.create(
					sd_utils::uuid_to_bytes(generate_pub_id()),
					vec![
						name::set(Some(self.name)),
						rules_per_kind::set(Some(rules_data)),
						date_created::set(Some(date_created.into())),
						date_modified::set(Some(date_created.into())),
					],
				)
				.exec()
				.await
				.map_err(|err| format!("database error: {err:#?}"))?,
		))
	}
}

pub fn generate_pub_id() -> Uuid {
	loop {
		let pub_id = Uuid::new_v4();
		if pub_id.as_u128() >= 0xFFF {
			return pub_id;
		}
	}
}
