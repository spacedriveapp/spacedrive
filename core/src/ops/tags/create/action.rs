//! Create a tag definition.
//!
//! A new definition has no source to live in until something applies it, so
//! it waits in the library's staging table. Creation is idempotent by slug:
//! the same path names the same tag, so creating `Work` twice returns the
//! existing definition rather than minting a rival uuid.

use super::{input::CreateTagInput, output::CreateTagOutput};
use crate::{
	context::CoreContext,
	domain::Tag,
	infra::action::{error::ActionError, LibraryAction},
	library::Library,
	ops::tags::{definitions, stamp},
};
use sd_store::{normalize_tag_path, slug_for_path, TagDefinition};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTagAction {
	input: CreateTagInput,
}

impl LibraryAction for CreateTagAction {
	type Input = CreateTagInput;
	type Output = CreateTagOutput;

	fn from_input(input: CreateTagInput) -> Result<Self, String> {
		input.validate()?;
		Ok(Self { input })
	}

	async fn execute(
		self,
		library: Arc<Library>,
		context: Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let cache = context.ephemeral_cache();
		let path = normalize_tag_path(&self.input.path)
			.map_err(|e| ActionError::InvalidInput(e.to_string()))?;
		let slug = slug_for_path(&path);

		if let Some(existing) = definitions::find_by_slug(&library, &cache, slug).await {
			return Ok(CreateTagOutput {
				tag: Tag::from_definition(&existing),
				created: false,
			});
		}

		let device = context
			.device_manager
			.device_id()
			.map_err(|e| ActionError::Internal(format!("no device identity: {e}")))?;
		let stamp = stamp::assertion_stamp(device);

		let definition = TagDefinition {
			uuid: Uuid::now_v7(),
			slug_id: slug,
			path,
			color: self.input.color.clone(),
			icon: self.input.icon.clone(),
			updated_hlc: stamp.hlc,
			origin_device: device,
		};

		definitions::stage(&library, &definition)
			.await
			.map_err(|e| ActionError::Internal(format!("could not stage the tag: {e}")))?;

		Ok(CreateTagOutput {
			tag: Tag::from_definition(&definition),
			created: true,
		})
	}

	fn action_kind(&self) -> &'static str {
		"tags.create"
	}
}

crate::register_library_action!(CreateTagAction, "tags.create");
