use sd_prisma::prisma::file_path;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Debug, Serialize, Deserialize, Hash, PartialEq, Eq, specta::Type)]
#[serde(transparent)]
pub struct CasId<'cas_id>(Cow<'cas_id, str>);

impl Clone for CasId<'_> {
	fn clone(&self) -> CasId<'static> {
		CasId(Cow::Owned(self.0.clone().into_owned()))
	}
}

impl CasId<'_> {
	#[must_use]
	pub fn as_str(&self) -> &str {
		self.0.as_ref()
	}

	#[must_use]
	pub fn to_owned(&self) -> CasId<'static> {
		CasId(Cow::Owned(self.0.clone().into_owned()))
	}

	#[must_use]
	pub fn into_owned(self) -> CasId<'static> {
		CasId(Cow::Owned(self.0.clone().into_owned()))
	}
}

impl From<&CasId<'_>> for file_path::cas_id::Type {
	fn from(CasId(cas_id): &CasId<'_>) -> Self {
		Some(cas_id.clone().into_owned())
	}
}

impl<'cas_id> From<&'cas_id str> for CasId<'cas_id> {
	fn from(cas_id: &'cas_id str) -> Self {
		Self(Cow::Borrowed(cas_id))
	}
}

impl<'cas_id> From<&'cas_id String> for CasId<'cas_id> {
	fn from(cas_id: &'cas_id String) -> Self {
		Self(Cow::Borrowed(cas_id))
	}
}

impl From<String> for CasId<'static> {
	fn from(cas_id: String) -> Self {
		Self(cas_id.into())
	}
}

impl From<CasId<'_>> for String {
	fn from(CasId(cas_id): CasId<'_>) -> Self {
		cas_id.into_owned()
	}
}

impl From<&CasId<'_>> for String {
	fn from(CasId(cas_id): &CasId<'_>) -> Self {
		cas_id.clone().into_owned()
	}
}
