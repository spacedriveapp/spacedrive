use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone)]
pub struct GetCurrentLibraryQuery;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetCurrentLibraryOutput {
	pub library_id: Option<uuid::Uuid>,
}

impl crate::cqrs::CoreQuery for GetCurrentLibraryQuery {
	type Input = ();
	type Output = GetCurrentLibraryOutput;

	fn from_input(input: Self::Input) -> anyhow::Result<Self> {
		Ok(Self)
	}

	async fn execute(
		self,
		context: std::sync::Arc<crate::context::CoreContext>,
	) -> anyhow::Result<Self::Output> {
		let session_state = context.session.get().await;
		Ok(GetCurrentLibraryOutput {
			library_id: session_state.current_library_id,
		})
	}
}

crate::register_core_query!(GetCurrentLibraryQuery, "libraries.session.current");
