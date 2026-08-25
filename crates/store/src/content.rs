//! Content identity: the convergent uuid derived from a record's bytes.
//!
//! A record's identity is assigned at discovery and rebound by evidence. Its
//! content's identity is *derived*, so two machines that have never
//! communicated compute the same id for the same bytes, offline, retroactively,
//! with no coordination and no clock. That is what makes a content id an
//! address rather than a local handle, and it is the only rebind evidence that
//! survives crossing to another device: path is weak, and inode does not travel
//! at all.
//!
//! The hash is a ladder, so the id is too. A uuid derived from a sampled hash
//! names bytes that are *probably* the same; one derived from an integrity hash
//! names bytes that are the same. [`ContentId`] carries which, because the
//! rules that matter (never delete one copy of two on the strength of a guess)
//! depend on the distinction, and a comment cannot enforce it.

use uuid::Uuid;

/// Namespace for content uuids, `v5(DNS, "content.spacedrive.app")`.
///
/// Every content id in every library derives from this constant. Changing it
/// changes every id, which is the same as declaring that no two installs have
/// ever seen the same file.
pub const CONTENT_NAMESPACE: Uuid = Uuid::from_bytes([
	0x33, 0x97, 0xca, 0x81, 0xf5, 0x79, 0x5e, 0xa0, 0x84, 0xd1, 0xec, 0x1d, 0x29, 0xee, 0xd5, 0x85,
]);

/// The uuid naming a hash. Same hash in, same uuid out, everywhere, forever.
pub fn uuid_for(hash: &str) -> Uuid {
	Uuid::new_v5(&CONTENT_NAMESPACE, hash.as_bytes())
}

/// A content uuid and how much the hash behind it is worth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContentId {
	/// Derived from a sampled hash. Two records carrying the same candidate
	/// are probably the same bytes.
	Candidate(Uuid),
	/// Derived from an integrity hash over every byte.
	Confirmed(Uuid),
}

impl ContentId {
	/// The uuid, whichever tier produced it. Use [`Self::confirmed`] where the
	/// answer decides whether bytes are removed.
	pub fn uuid(&self) -> Uuid {
		match self {
			Self::Candidate(u) | Self::Confirmed(u) => *u,
		}
	}

	/// The uuid, only when it came from an integrity hash.
	pub fn confirmed(&self) -> Option<Uuid> {
		match self {
			Self::Confirmed(u) => Some(*u),
			Self::Candidate(_) => None,
		}
	}

	/// Derive from the best hash available, preferring the integrity tier.
	/// `None` when neither hash has landed yet.
	pub fn from_hashes(sampled: Option<&str>, integrity: Option<&str>) -> Option<Self> {
		match (integrity, sampled) {
			(Some(h), _) => Some(Self::Confirmed(uuid_for(h))),
			(None, Some(h)) => Some(Self::Candidate(uuid_for(h))),
			(None, None) => None,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn same_hash_yields_same_uuid() {
		assert_eq!(uuid_for("abc123"), uuid_for("abc123"));
		assert_ne!(uuid_for("abc123"), uuid_for("abc124"));
	}

	#[test]
	fn namespace_is_pinned() {
		// Guards against an accidental edit to the constant, which would
		// silently invalidate every content id ever computed.
		assert_eq!(
			uuid_for("hello").to_string(),
			"3d902c01-9450-5d4c-9169-08f7d3f05b2a"
		);
	}

	#[test]
	fn integrity_wins_over_sampled() {
		let id = ContentId::from_hashes(Some("sampled"), Some("integrity")).unwrap();
		assert_eq!(id, ContentId::Confirmed(uuid_for("integrity")));
		assert!(id.confirmed().is_some());
	}

	#[test]
	fn sampled_alone_is_a_candidate() {
		let id = ContentId::from_hashes(Some("sampled"), None).unwrap();
		assert_eq!(id, ContentId::Candidate(uuid_for("sampled")));
		assert!(id.confirmed().is_none());
	}

	#[test]
	fn no_hash_is_no_id() {
		assert!(ContentId::from_hashes(None, None).is_none());
	}
}
