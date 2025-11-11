use super::{input::SpaceCreateInput, output::SpaceCreateOutput};
use crate::{
	context::CoreContext,
	domain::Space,
	infra::action::{
		error::{ActionError, ActionResult},
		LibraryAction,
	},
};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, EntityTrait, QueryOrder, Set};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceCreateAction {
	input: SpaceCreateInput,
}

impl SpaceCreateAction {
	pub fn new(input: SpaceCreateInput) -> Self {
		Self { input }
	}
}

impl LibraryAction for SpaceCreateAction {
	type Input = SpaceCreateInput;
	type Output = SpaceCreateOutput;

	fn from_input(input: SpaceCreateInput) -> Result<Self, String> {
		// Validate input
		if input.name.trim().is_empty() {
			return Err("Space name cannot be empty".to_string());
		}

		if !Space::validate_color(&input.color) {
			return Err("Invalid color format. Must be #RRGGBB".to_string());
		}

		Ok(SpaceCreateAction::new(input))
	}

	async fn execute(
		self,
		library: std::sync::Arc<crate::library::Library>,
		_context: std::sync::Arc<CoreContext>,
	) -> Result<Self::Output, ActionError> {
		let db = library.db().conn();

		// Get the current max order
		let max_order = crate::infra::db::entities::space::Entity::find()
			.order_by_desc(crate::infra::db::entities::space::Column::Order)
			.one(db)
			.await
			.map_err(ActionError::SeaOrm)?
			.map(|s| s.order)
			.unwrap_or(-1);

		let space_id = uuid::Uuid::new_v4();
		let now = Utc::now();

		// Create space entity
		let active_model = crate::infra::db::entities::space::ActiveModel {
			id: Set(0), // Auto-increment
			uuid: Set(space_id),
			name: Set(self.input.name.clone()),
			icon: Set(self.input.icon.clone()),
			color: Set(self.input.color.clone()),
			order: Set(max_order + 1),
			created_at: Set(now.into()),
			updated_at: Set(now.into()),
		};

		let result = active_model.insert(db).await.map_err(ActionError::SeaOrm)?;

		let space = Space {
			id: result.uuid,
			name: result.name,
			icon: result.icon,
			color: result.color,
			order: result.order,
			created_at: result.created_at.into(),
			updated_at: result.updated_at.into(),
		};

		Ok(SpaceCreateOutput { space })
	}

	fn action_kind(&self) -> &'static str {
		"spaces.create"
	}

	async fn validate(
		&self,
		_library: std::sync::Arc<crate::library::Library>,
		_context: std::sync::Arc<CoreContext>,
	) -> ActionResult<()> {
		Ok(())
	}
}

crate::register_library_action!(SpaceCreateAction, "spaces.create");
