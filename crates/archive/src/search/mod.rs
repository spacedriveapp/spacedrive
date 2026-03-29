//! Unified search types and query routing.

pub mod fts;
pub mod router;
pub mod vector;

use serde::{Deserialize, Serialize};

use crate::safety::TrustTier;

/// A single search result from any source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
	pub id: String,
	pub title: String,
	pub preview: String,
	pub subtitle: Option<String>,
	pub snippet: Option<String>,
	pub rank: f64,
	pub source_id: String,
	pub source_name: String,
	pub data_type: String,
	pub data_type_icon: Option<String>,
	pub date: Option<String>,
	pub trust_tier: TrustTier,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub safety_verdict: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub safety_score: Option<u8>,
}

/// Filters for search queries.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchFilter {
	pub source_id: Option<String>,
	pub data_type: Option<String>,
	pub limit: Option<usize>,
	pub date_after: Option<String>,
	pub date_before: Option<String>,
	#[serde(default)]
	pub sort_by_date: bool,
}
