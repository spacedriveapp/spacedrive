//! Create semantic tag action

use super::{input::CreateTagInput, output::CreateTagOutput};
use crate::infra::sync::ChangeType;
use crate::{
	context::CoreContext,
	domain::tag::{PrivacyLevel, Tag, TagType},
	infra::action::{error::ActionError, LibraryAction},
	library::Library,
	ops::tags::manager::TagManager,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTagAction {
	input: CreateTagInput,
}

impl CreateTagAction {
	pub fn new(input: CreateTagInput) -> Self {
		Self { input }
	}
}

impl LibraryAction for CreateTagAction {
	type Input = CreateTagInput;
	type Output = CreateTagOutput;

	fn from_input(input: CreateTagInput) -> Result<Self, String> {
		input.validate()?;
		Ok(CreateTagAction::new(input))
	}

	async fn execute(
		self,
		library: Arc<Library>,
		_context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let db = library.db();
		let semantic_tag_manager = TagManager::new(Arc::new(db.conn().clone()));

		// Get current device ID from library context
		let device_id = library.id(); // Use library ID as device ID

		// Create the semantic tag with all optional fields
		let tag_entity = semantic_tag_manager
			.create_tag_entity_full(
				self.input.canonical_name.clone(),
				self.input.namespace.clone(),
				self.input.display_name.clone(),
				self.input.formal_name.clone(),
				self.input.abbreviation.clone(),
				self.input.aliases.clone(),
				self.input.tag_type,
				self.input.color.clone(),
				self.input.icon.clone(),
				self.input.description.clone(),
				self.input.is_organizational_anchor.unwrap_or(false),
				self.input.privacy_level,
				self.input.search_weight,
				self.input.attributes.clone(),
				device_id,
			)
			.await
			.map_err(|e| ActionError::Internal(format!("Failed to create tag: {}", e)))?;

		library
			.sync_model(&tag_entity, ChangeType::Insert)
			.await
			.map_err(|e| ActionError::Internal(format!("Failed to sync tag: {}", e)))?;

		Ok(CreateTagOutput::from_entity(&tag_entity))
	}

	fn action_kind(&self) -> &'static str {
		"tags.create"
	}
}

// Register library action
crate::register_library_action!(CreateTagAction, "tags.create");
