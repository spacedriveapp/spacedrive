#[allow(warnings, unused)]
pub mod prisma;
#[allow(warnings, unused)]
pub mod prisma_sync;

macro_rules! impl_model {
	($module:ident) => {
		impl sd_cache::Model for prisma::$module::Data {
			fn name() -> &'static str {
				prisma::$module::NAME
			}
		}
	};
}

impl_model!(tag);
impl_model!(object);
impl_model!(location);
impl_model!(indexer_rule);
impl_model!(file_path);

pub async fn test_db() -> std::sync::Arc<prisma::PrismaClient> {
	std::sync::Arc::new(
		prisma::PrismaClient::_builder()
			.with_url(format!("file:/tmp/test-db-{}", uuid::Uuid::new_v4()))
			.build()
			.await
			.unwrap(),
	)
}
