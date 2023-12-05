#[allow(warnings, unused)]
pub mod prisma;
#[allow(warnings, unused)]
pub mod prisma_sync;

impl sd_cache::Model for prisma::tag::Data {
	fn name() -> &'static str {
		"Tag"
	}
}

impl sd_cache::Model for prisma::object::Data {
	fn name() -> &'static str {
		"Object"
	}
}

impl sd_cache::Model for prisma::location::Data {
	fn name() -> &'static str {
		"Location"
	}
}

impl sd_cache::Model for prisma::indexer_rule::Data {
	fn name() -> &'static str {
		"IndexerRule"
	}
}

impl sd_cache::Model for prisma::file_path::Data {
	fn name() -> &'static str {
		"FilePath"
	}
}
