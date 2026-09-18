//! The stamp on a locally authored assertion.
//!
//! Every tag apply, remove, and definition write carries an HLC and the
//! device that wrote it, because assertion state is resolved by ordering
//! rather than by whoever wrote last on this machine. One process-wide clock
//! keeps local stamps monotonic; merges feed received stamps back through
//! [`observe_remote`] so later local claims sort after everything this device
//! has seen.

use std::sync::{Arc, OnceLock};

use sd_store::Stamp;
use uuid::Uuid;

use crate::infra::sync::time_source::SystemTimeSource;
use crate::infra::sync::{HLCGenerator, HLC};

static CLOCK: OnceLock<HLCGenerator> = OnceLock::new();

fn clock(device: Uuid) -> &'static HLCGenerator {
	CLOCK.get_or_init(|| HLCGenerator::new(device, Arc::new(SystemTimeSource)))
}

/// One stamp for one authored claim. Rows written by the same action share a
/// stamp: they are one event, and the assertion primary key separates them by
/// tag and record.
pub fn assertion_stamp(device: Uuid) -> Stamp {
	Stamp {
		hlc: clock(device).next().to_string(),
		device_uuid: device,
	}
}

/// Fold a received HLC into the clock, so a removal authored here after a
/// merge sorts after the applies that arrived in it.
pub fn observe_remote(device: Uuid, received: HLC) {
	clock(device).update(received);
}
