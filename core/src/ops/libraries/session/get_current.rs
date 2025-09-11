#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GetCurrentLibraryQuery;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GetCurrentLibraryOutput { pub library_id: Option<uuid::Uuid> }

impl crate::cqrs::Query for GetCurrentLibraryQuery {
	type Output = GetCurrentLibraryOutput;

	async fn execute(self, context: std::sync::Arc<crate::context::CoreContext>) -> anyhow::Result<Self::Output> {
		// Placeholder: without daemon session service here, return None
		Ok(GetCurrentLibraryOutput { library_id: None })
	}
}

crate::register_query!(GetCurrentLibraryQuery, "libraries.session.current");

