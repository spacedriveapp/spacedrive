//! Queue a video scrub sheet for the path a client is interacting with.

use std::{path::PathBuf, sync::Arc};

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
	context::CoreContext,
	domain::SdPath,
	infra::action::{error::ActionError, CoreAction},
	service::thumbs::ThumbstripIdentity,
};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ThumbstripRequestInput {
	pub path: SdPath,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ThumbstripRequestOutput {
	pub thumbstrip: Option<ThumbstripIdentity>,
}

pub struct ThumbstripRequestAction {
	path: PathBuf,
}

impl CoreAction for ThumbstripRequestAction {
	type Input = ThumbstripRequestInput;
	type Output = ThumbstripRequestOutput;

	fn from_input(input: Self::Input) -> Result<Self, String> {
		match input.path {
			SdPath::Physical { device_slug, path }
				if device_slug == crate::device::get_current_device_slug() =>
			{
				Ok(Self { path })
			}
			SdPath::Physical { device_slug, .. } => {
				Err(format!("{device_slug} does not name this device"))
			}
			other => Err(format!("{other:?} does not name a local file")),
		}
	}

	async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output, ActionError> {
		Ok(ThumbstripRequestOutput {
			thumbstrip: context.thumbs.request_thumbstrip(&self.path).await,
		})
	}

	fn action_kind(&self) -> &'static str {
		"thumbstrips.request"
	}
}

crate::register_core_action!(ThumbstripRequestAction, "thumbstrips.request");
