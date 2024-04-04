use serde::{Deserialize, Serialize};
use specta::Type;

#[repr(i32)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, Eq, PartialEq, Hash)]
pub enum RuleKind {
	AcceptFilesByGlob = 0,
	RejectFilesByGlob = 1,
	AcceptIfChildrenDirectoriesArePresent = 2,
	RejectIfChildrenDirectoriesArePresent = 3,
}

impl RuleKind {
	pub const fn variant_count() -> usize {
		// TODO: Use https://doc.rust-lang.org/std/mem/fn.variant_count.html if it ever gets stabilized
		4
	}
}
