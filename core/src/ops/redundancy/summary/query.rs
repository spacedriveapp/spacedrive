//! Redundancy summary query implementation

use super::{
	input::RedundancySummaryInput,
	output::{LibraryRedundancyTotals, RedundancySummaryOutput, VolumeRedundancySummary},
};
use crate::{
	context::CoreContext,
	infra::{
		db::entities,
		query::{LibraryQuery, QueryError, QueryResult},
	},
};
use sea_orm::{
	ColumnTrait, ConnectionTrait, DbBackend, EntityTrait, FromQueryResult, QueryFilter, Statement,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;

/// Redundancy summary query
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RedundancySummaryQuery {
	pub input: RedundancySummaryInput,
}

/// Row result for per-volume redundancy stats
#[derive(FromQueryResult)]
struct VolumeRedundancyRow {
	volume_id: i32,
	file_count: i64,
	content_bytes: i64,
}

/// Row result for per-volume total file stats
#[derive(FromQueryResult)]
struct VolumeTotalRow {
	volume_id: i32,
	total_file_count: i64,
	total_bytes: i64,
}

/// Row result for library-wide unique content totals
#[derive(FromQueryResult)]
struct LibraryTotalRow {
	total_bytes: i64,
}

impl LibraryQuery for RedundancySummaryQuery {
	type Input = RedundancySummaryInput;
	type Output = RedundancySummaryOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let library_id = session
			.current_library_id
			.ok_or_else(|| QueryError::Internal("No library in session".to_string()))?;

		let library = context
			.libraries()
			.await
			.get_library(library_id)
			.await
			.ok_or_else(|| QueryError::Internal("Library not found".to_string()))?;

		let db = library.db().conn();

		// Build volume UUID filter if scoped
		let volume_id_filter = if let Some(ref uuids) = self.input.volume_uuids {
			if uuids.is_empty() {
				None
			} else {
				// Resolve UUIDs to internal IDs
				let volumes = entities::volume::Entity::find()
					.filter(entities::volume::Column::Uuid.is_in(uuids.clone()))
					.all(db)
					.await?;
				Some(volumes.iter().map(|v| v.id).collect::<Vec<i32>>())
			}
		} else {
			None
		};

		// Fetch all visible volumes for display names and UUID mapping
		let all_volumes = entities::volume::Entity::find()
			.filter(entities::volume::Column::IsUserVisible.eq(true))
			.all(db)
			.await?;

		let volume_id_to_uuid: HashMap<i32, Uuid> =
			all_volumes.iter().map(|v| (v.id, v.uuid)).collect();
		let volume_id_to_name: HashMap<i32, Option<String>> = all_volumes
			.iter()
			.map(|v| (v.id, v.display_name.clone()))
			.collect();

		// Helper to build volume ID WHERE clause
		let volume_where = match &volume_id_filter {
			Some(ids) => {
				let id_list = ids
					.iter()
					.map(|id| id.to_string())
					.collect::<Vec<_>>()
					.join(",");
				format!("AND e.volume_id IN ({})", id_list)
			}
			None => String::new(),
		};

		// Query 1: Per-volume at-risk content (content existing on exactly one volume)
		let at_risk_sql = format!(
			r#"
			SELECT e.volume_id as volume_id,
			       COUNT(*) as file_count,
			       COALESCE(SUM(ci.total_size), 0) as content_bytes
			FROM entries e
			INNER JOIN content_identities ci ON e.content_id = ci.id
			WHERE e.content_id IS NOT NULL
			  AND e.volume_id IS NOT NULL
			  AND e.kind = 0
			  {}
			  AND e.content_id IN (
			      SELECT e2.content_id FROM entries e2
			      WHERE e2.content_id IS NOT NULL AND e2.volume_id IS NOT NULL
			      GROUP BY e2.content_id
			      HAVING COUNT(DISTINCT e2.volume_id) = 1
			  )
			GROUP BY e.volume_id
			"#,
			volume_where
		);

		let at_risk_rows = VolumeRedundancyRow::find_by_statement(Statement::from_string(
			DbBackend::Sqlite,
			at_risk_sql,
		))
		.all(db)
		.await?;

		// Query 2: Per-volume redundant content (content existing on 2+ volumes)
		let redundant_sql = format!(
			r#"
			SELECT e.volume_id as volume_id,
			       COUNT(*) as file_count,
			       COALESCE(SUM(ci.total_size), 0) as content_bytes
			FROM entries e
			INNER JOIN content_identities ci ON e.content_id = ci.id
			WHERE e.content_id IS NOT NULL
			  AND e.volume_id IS NOT NULL
			  AND e.kind = 0
			  {}
			  AND e.content_id IN (
			      SELECT e2.content_id FROM entries e2
			      WHERE e2.content_id IS NOT NULL AND e2.volume_id IS NOT NULL
			      GROUP BY e2.content_id
			      HAVING COUNT(DISTINCT e2.volume_id) > 1
			  )
			GROUP BY e.volume_id
			"#,
			volume_where
		);

		let redundant_rows = VolumeRedundancyRow::find_by_statement(Statement::from_string(
			DbBackend::Sqlite,
			redundant_sql,
		))
		.all(db)
		.await?;

		// Query 3: Per-volume total file counts
		let totals_sql = format!(
			r#"
			SELECT e.volume_id as volume_id,
			       COUNT(*) as total_file_count,
			       COALESCE(SUM(e.size), 0) as total_bytes
			FROM entries e
			WHERE e.volume_id IS NOT NULL
			  AND e.kind = 0
			  {}
			GROUP BY e.volume_id
			"#,
			volume_where
		);

		let total_rows = VolumeTotalRow::find_by_statement(Statement::from_string(
			DbBackend::Sqlite,
			totals_sql,
		))
		.all(db)
		.await?;

		// Query 4: Library-wide unique content total (deduplicated)
		let library_total_sql = r#"
			SELECT COALESCE(SUM(ci.total_size), 0) as total_bytes
			FROM content_identities ci
			WHERE ci.id IN (
			    SELECT DISTINCT e.content_id FROM entries e
			    WHERE e.content_id IS NOT NULL AND e.volume_id IS NOT NULL
			)
		"#;

		let library_total = LibraryTotalRow::find_by_statement(Statement::from_string(
			DbBackend::Sqlite,
			library_total_sql.to_string(),
		))
		.one(db)
		.await?;

		// Build lookup maps
		let mut at_risk_map: HashMap<i32, &VolumeRedundancyRow> = HashMap::new();
		for row in &at_risk_rows {
			at_risk_map.insert(row.volume_id, row);
		}

		let mut redundant_map: HashMap<i32, &VolumeRedundancyRow> = HashMap::new();
		for row in &redundant_rows {
			redundant_map.insert(row.volume_id, row);
		}

		let mut totals_map: HashMap<i32, &VolumeTotalRow> = HashMap::new();
		for row in &total_rows {
			totals_map.insert(row.volume_id, row);
		}

		// Build per-volume summaries
		let mut volumes = Vec::new();
		let mut lib_at_risk_bytes: i64 = 0;
		let mut lib_redundant_bytes: i64 = 0;

		// Determine which volume IDs to include
		let volume_ids: Vec<i32> = match &volume_id_filter {
			Some(ids) => ids.clone(),
			None => {
				let mut ids: Vec<i32> = totals_map.keys().copied().collect();
				ids.sort();
				ids
			}
		};

		for vol_id in &volume_ids {
			let volume_uuid = match volume_id_to_uuid.get(vol_id) {
				Some(uuid) => *uuid,
				None => continue,
			};

			let at_risk = at_risk_map.get(vol_id);
			let redundant = redundant_map.get(vol_id);
			let total = totals_map.get(vol_id);

			let at_risk_bytes = at_risk.map_or(0, |r| r.content_bytes);
			let at_risk_file_count = at_risk.map_or(0, |r| r.file_count as u32);
			let redundant_bytes = redundant.map_or(0, |r| r.content_bytes);
			let redundant_file_count = redundant.map_or(0, |r| r.file_count as u32);
			let total_bytes = total.map_or(0, |r| r.total_bytes);
			let total_file_count = total.map_or(0, |r| r.total_file_count as u32);

			lib_at_risk_bytes += at_risk_bytes;
			lib_redundant_bytes += redundant_bytes;

			volumes.push(VolumeRedundancySummary {
				volume_uuid,
				display_name: volume_id_to_name.get(vol_id).cloned().flatten(),
				total_bytes,
				at_risk_bytes,
				at_risk_file_count,
				redundant_bytes,
				redundant_file_count,
				total_file_count,
			});
		}

		let total_unique_content_bytes = library_total.map_or(0, |r| r.total_bytes);
		let replication_score = if lib_at_risk_bytes + lib_redundant_bytes > 0 {
			lib_redundant_bytes as f64 / (lib_at_risk_bytes + lib_redundant_bytes) as f64
		} else {
			0.0
		};

		Ok(RedundancySummaryOutput {
			volumes,
			library_totals: LibraryRedundancyTotals {
				total_unique_content_bytes,
				total_at_risk_bytes: lib_at_risk_bytes,
				total_redundant_bytes: lib_redundant_bytes,
				replication_score,
			},
		})
	}
}

crate::register_library_query!(RedundancySummaryQuery, "redundancy.summary");
