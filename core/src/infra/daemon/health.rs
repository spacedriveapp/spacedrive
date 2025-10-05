use crate::infra::daemon::types::{DaemonRequest, DaemonResponse};

pub fn version_string() -> String {
	format!("{}", env!("CARGO_PKG_VERSION"))
}
