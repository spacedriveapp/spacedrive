#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GetCurrentLibraryQuery;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GetCurrentLibraryOutput {
	pub library_id: Option<uuid::Uuid>,
}

impl crate::cqrs::Query for GetCurrentLibraryQuery {
	type Output = GetCurrentLibraryOutput;

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

crate::register_query!(GetCurrentLibraryQuery, "libraries.session.current");
