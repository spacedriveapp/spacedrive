use crate::library::LibraryManagerError;
use prisma_client_rust::raw;
use sd_prisma::prisma::PrismaClient;
use tracing::{error, info};

// **************************************
// pragmas to optimize SQLite performance
// **************************************

#[derive(serde::Deserialize, Debug)]
#[allow(dead_code)]
struct JournalMode {
	journal_mode: String,
}

#[derive(serde::Deserialize, Debug)]
#[allow(dead_code)]
struct CacheSize {
	cache_size: i32,
}

#[derive(serde::Deserialize, Debug)]
#[allow(dead_code)]
struct MmapSize {
	mmap_size: i64,
}

#[derive(serde::Deserialize, Debug)]
#[allow(dead_code)]
struct JournalSizeLimit {
	journal_size_limit: i64,
}

async fn execute_pragma<T: serde::de::DeserializeOwned + 'static>(
	db: &PrismaClient,
	query: &str,
	description: &str,
) -> Result<(), LibraryManagerError> {
	match db._query_raw::<T>(raw!(query)).exec().await {
		Ok(_) => {
			info!("{}", description);
			Ok(())
		}
		Err(e) => {
			error!("Failed to execute '{}': {:?}", description, e);
			Err(e.into())
		}
	}
}

pub async fn configure_pragmas(db: &PrismaClient) -> Result<(), LibraryManagerError> {
	let pragmas = vec![
		(
			// WAL (Write-Ahead Logging) mode allows SQLite to perform better in situations with concurrent reads and writes.
			// It uses a separate write-ahead log file instead of overwriting the database file directly, enabling better performance and durability.
			// This mode is commonly used to optimize database performance in environments where multiple transactions happen simultaneously.
			"PRAGMA journal_mode = WAL;",
			"Set journal mode to WAL",
			Some("JournalMode"),
		),
		(
			// The synchronous mode controls how often SQLite waits for data to be physically written to disk.
			// Setting it to NORMAL means SQLite will wait for the data to be flushed to the disk at key points but not after every transaction.
			// This mode balances durability with performance. It's faster than FULL (which waits on every transaction) and safer than OFF.
			"PRAGMA synchronous = NORMAL;",
			"Set synchronous to NORMAL",
			None,
		),
		(
			// mmap_size sets the maximum number of bytes that SQLite will map into memory when using memory-mapped I/O.
			// 512MB (536870912 bytes) is a reasonable amount for systems with enough RAM, allowing SQLite to access data directly from memory, improving performance for large databases.
			// A larger mmap_size typically results in better performance, especially for read-heavy operations, but it should not exceed available system memory.
			"PRAGMA mmap_size = 536870912;",
			"Set mmap_size to 512MB",
			Some("MmapSize"),
		),
		(
			// journal_size_limit sets a maximum size for the write-ahead log (WAL) file.
			// Limiting it to 64MB (67108864 bytes) ensures that the WAL file doesn’t grow too large, which could otherwise consume excessive disk space.
			// A smaller limit might cause SQLite to checkpoint (merge the WAL file back into the database) more often, which could slightly impact performance.
			"PRAGMA journal_size_limit = 67108864;",
			"Set journal size limit to 64MB",
			Some("JournalSizeLimit"),
		),
		(
			// cache_size defines how much memory SQLite should allocate for storing frequently accessed database pages in memory.
			// Setting it to 10,000 pages allows SQLite to cache more pages in RAM, reducing the need to hit the disk frequently.
			// Each page is typically 4KB, so 10,000 pages would equal around 40MB of cache, which can significantly improve performance for frequently accessed data.
			"PRAGMA cache_size = 10000;",
			"Set cache size to 10k pages",
			Some("CacheSize"),
		),
	];

	for (query, description, result_type) in pragmas {
		match result_type {
			Some("JournalMode") => execute_pragma::<JournalMode>(db, query, description).await?,
			Some("CacheSize") => execute_pragma::<CacheSize>(db, query, description).await?,
			Some("MmapSize") => execute_pragma::<MmapSize>(db, query, description).await?,
			Some("JournalSizeLimit") => {
				execute_pragma::<JournalSizeLimit>(db, query, description).await?
			}
			None => execute_pragma::<()>(db, query, description).await?,
			_ => unreachable!(),
		}
	}

	Ok(())
}
