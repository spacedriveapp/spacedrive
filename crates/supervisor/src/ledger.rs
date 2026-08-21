//! The host's port ledger: deterministic port allocation over an explicit,
//! operator-editable file. Logical port requests (holder identity, policy,
//! preferred port) are knowledge; the concrete host port is a machine fact
//! recorded as a lease. The allocator is pure and all-or-nothing per batch;
//! the store owns the file and arbitrates in-process, so racing planners are
//! serialized rather than fenced.

use serde::{Deserialize, Serialize};
use specta::Type;
use std::{
	io,
	path::{Path, PathBuf},
};
use tokio::sync::Mutex;

/// Default allocatable ranges for a new ledger. Explicitly a starting point,
/// written into the file where the operator can change it; nothing else in
/// the system assumes these numbers.
const DEFAULT_ALLOCATABLE: [PortRange; 4] = [
	PortRange { from: 3000, to: 3999 },
	PortRange { from: 5000, to: 5999 },
	PortRange { from: 8000, to: 8999 },
	PortRange {
		from: 42000,
		to: 42099,
	},
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct PortRange {
	pub from: u16,
	pub to: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PortLeaseHolder {
	pub installation_id: String,
	pub endpoint_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum PortPolicy {
	/// Exactly the preferred port or refusal: protocol-pinned services and
	/// explicit operator overrides.
	Fixed,
	/// Preferred first, then the lowest free allocatable port.
	Remappable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum LeaseSource {
	/// This allocator issued it.
	Allocated,
	/// Adopted from an app's own declaration.
	Declared,
	/// A live listener adopted as occupancy.
	Observed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct PortLease {
	pub port: u16,
	pub holder: PortLeaseHolder,
	pub policy: PortPolicy,
	pub source: LeaseSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PortLedger {
	pub schema_version: String,
	/// Monotonic; every successful allocation is a new generation.
	pub generation: u64,
	/// Explicit ranges remappable requests may draw from.
	pub allocatable: Vec<PortRange>,
	/// Never assigned, whatever asks.
	pub excluded: Vec<PortRange>,
	pub leases: Vec<PortLease>,
}

impl Default for PortLedger {
	fn default() -> Self {
		Self {
			schema_version: "port-ledger/1".to_string(),
			generation: 0,
			allocatable: DEFAULT_ALLOCATABLE.to_vec(),
			excluded: Vec::new(),
			leases: Vec::new(),
		}
	}
}

#[derive(Debug, Clone)]
pub struct PortRequest {
	pub installation_id: String,
	pub endpoint_id: String,
	pub policy: PortPolicy,
	pub preferred_port: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortAssignment {
	pub installation_id: String,
	pub endpoint_id: String,
	pub port: u16,
}

#[derive(Debug)]
pub enum PortResolution {
	Ok {
		assignments: Vec<PortAssignment>,
		ledger: PortLedger,
	},
	Conflict(Vec<String>),
}

fn in_ranges(ranges: &[PortRange], port: u16) -> bool {
	ranges
		.iter()
		.any(|range| port >= range.from && port <= range.to)
}

fn holder_key(installation_id: &str, endpoint_id: &str) -> String {
	format!("{installation_id}/{endpoint_id}")
}

/// Deterministic, atomic allocation for one batch of requests. Existing
/// leases held by a requesting (installation, endpoint) are preserved
/// verbatim: re-resolution never moves a settled port. Any conflict refuses
/// the whole batch; the input ledger is never mutated.
pub fn resolve_port_assignments(ledger: &PortLedger, requests: &[PortRequest]) -> PortResolution {
	use std::collections::HashMap;

	let mut conflicts: Vec<String> = Vec::new();
	let taken: HashMap<u16, &PortLease> =
		ledger.leases.iter().map(|lease| (lease.port, lease)).collect();
	let by_holder: HashMap<String, &PortLease> = ledger
		.leases
		.iter()
		.map(|lease| {
			(
				holder_key(&lease.holder.installation_id, &lease.holder.endpoint_id),
				lease,
			)
		})
		.collect();
	let mut claimed: HashMap<u16, String> = HashMap::new();
	let mut assignments: Vec<PortAssignment> = Vec::new();
	let mut new_leases: Vec<PortLease> = Vec::new();

	// Fixed requests resolve first. A remappable request only states a
	// preference and has the whole allocatable range to fall back on, so
	// letting one claim a port some fixed request requires would refuse an
	// assignment that exists — and which of them asked first would come down
	// to how the installations happen to be named. Ordering within a policy
	// stays lexical so the outcome is deterministic.
	let mut ordered: Vec<&PortRequest> = requests.iter().collect();
	ordered.sort_by(|a, b| {
		let rank = |policy: PortPolicy| if policy == PortPolicy::Fixed { 0 } else { 1 };
		rank(a.policy)
			.cmp(&rank(b.policy))
			.then_with(|| a.installation_id.cmp(&b.installation_id))
			.then_with(|| a.endpoint_id.cmp(&b.endpoint_id))
	});

	let blocked_by = |port: u16, key: &str, claimed: &HashMap<u16, String>| -> Option<String> {
		if let Some(claimant) = claimed.get(&port) {
			if claimant != key {
				return Some(format!("claimed by {claimant} in this batch"));
			}
		}
		if let Some(lease) = taken.get(&port) {
			let lease_key = holder_key(&lease.holder.installation_id, &lease.holder.endpoint_id);
			if lease_key != key {
				return Some(format!("leased to {lease_key}"));
			}
		}
		if in_ranges(&ledger.excluded, port) {
			return Some("excluded on this host".to_string());
		}
		None
	};

	for request in ordered {
		let key = holder_key(&request.installation_id, &request.endpoint_id);
		if let Some(existing) = by_holder.get(&key) {
			claimed.insert(existing.port, key.clone());
			assignments.push(PortAssignment {
				installation_id: request.installation_id.clone(),
				endpoint_id: request.endpoint_id.clone(),
				port: existing.port,
			});
			continue;
		}

		if request.policy == PortPolicy::Fixed {
			let Some(port) = request.preferred_port else {
				conflicts.push(format!("{key}: a fixed request must name its port"));
				continue;
			};
			if let Some(blocked) = blocked_by(port, &key, &claimed) {
				conflicts.push(format!("{key}: fixed port {port} is {blocked}"));
				continue;
			}
			claimed.insert(port, key.clone());
			assignments.push(PortAssignment {
				installation_id: request.installation_id.clone(),
				endpoint_id: request.endpoint_id.clone(),
				port,
			});
			new_leases.push(PortLease {
				port,
				holder: PortLeaseHolder {
					installation_id: request.installation_id.clone(),
					endpoint_id: request.endpoint_id.clone(),
				},
				policy: PortPolicy::Fixed,
				source: LeaseSource::Allocated,
			});
			continue;
		}

		let mut port: Option<u16> = None;
		if let Some(preferred) = request.preferred_port {
			if in_ranges(&ledger.allocatable, preferred)
				&& blocked_by(preferred, &key, &claimed).is_none()
			{
				port = Some(preferred);
			}
		}
		if port.is_none() {
			let mut ranges = ledger.allocatable.clone();
			ranges.sort_by_key(|range| range.from);
			'ranges: for range in ranges {
				for candidate in range.from..=range.to {
					if blocked_by(candidate, &key, &claimed).is_none() {
						port = Some(candidate);
						break 'ranges;
					}
				}
			}
		}
		let Some(port) = port else {
			conflicts.push(format!("{key}: no allocatable port remains on this host"));
			continue;
		};
		claimed.insert(port, key.clone());
		assignments.push(PortAssignment {
			installation_id: request.installation_id.clone(),
			endpoint_id: request.endpoint_id.clone(),
			port,
		});
		new_leases.push(PortLease {
			port,
			holder: PortLeaseHolder {
				installation_id: request.installation_id.clone(),
				endpoint_id: request.endpoint_id.clone(),
			},
			policy: PortPolicy::Remappable,
			source: LeaseSource::Allocated,
		});
	}

	if !conflicts.is_empty() {
		return PortResolution::Conflict(conflicts);
	}
	let mut leases = ledger.leases.clone();
	leases.extend(new_leases);
	leases.sort_by_key(|lease| lease.port);
	PortResolution::Ok {
		assignments,
		ledger: PortLedger {
			generation: ledger.generation + 1,
			leases,
			..ledger.clone()
		},
	}
}

/// Owner of the ledger file. All allocation goes through one store instance,
/// so arbitration is the lock rather than a fence on the file.
#[derive(Debug)]
pub struct PortLedgerStore {
	path: PathBuf,
	state: Mutex<PortLedger>,
}

impl PortLedgerStore {
	/// Load the ledger, falling back to defaults when the file is missing or
	/// unreadable — the file is operator-editable, and a broken edit must not
	/// take supervision down with it.
	pub fn load(path: impl Into<PathBuf>) -> Self {
		let path = path.into();
		let ledger = std::fs::read_to_string(&path)
			.ok()
			.and_then(|raw| serde_json::from_str(&raw).ok())
			.unwrap_or_default();
		Self {
			path,
			state: Mutex::new(ledger),
		}
	}

	pub async fn ledger(&self) -> PortLedger {
		self.state.lock().await.clone()
	}

	/// Allocate a batch of requests and persist the resulting ledger.
	pub async fn allocate(
		&self,
		requests: &[PortRequest],
	) -> Result<Vec<PortAssignment>, Vec<String>> {
		let mut state = self.state.lock().await;
		match resolve_port_assignments(&state, requests) {
			PortResolution::Ok {
				assignments,
				ledger,
			} => {
				if ledger.generation != state.generation {
					if let Err(err) = persist(&self.path, &ledger) {
						return Err(vec![format!("failed to persist port ledger: {err}")]);
					}
				}
				*state = ledger;
				Ok(assignments)
			}
			PortResolution::Conflict(conflicts) => Err(conflicts),
		}
	}
}

fn persist(path: &Path, ledger: &PortLedger) -> io::Result<()> {
	if let Some(parent) = path.parent() {
		std::fs::create_dir_all(parent)?;
	}
	let temporary = path.with_extension("json.tmp");
	let mut body = serde_json::to_string_pretty(ledger).map_err(io::Error::other)?;
	body.push('\n');
	std::fs::write(&temporary, body)?;
	std::fs::rename(&temporary, path)
}
