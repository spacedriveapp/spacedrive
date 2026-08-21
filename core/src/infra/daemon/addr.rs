//! Socket address derivation for daemon instances.
//!
//! The daemon binary and every client must agree on which loopback port a
//! given instance listens on, so the derivation lives here where both sides
//! can reach it.

use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;

/// Port used by the default (unnamed) daemon instance.
const DEFAULT_PORT: u16 = 6969;

/// Base port for named instances; the instance name hashes into a
/// 1000-port range starting here.
const INSTANCE_PORT_BASE: u16 = 6970;

/// Resolve the loopback socket address a daemon instance listens on.
///
/// The default instance uses a fixed port. Named instances derive a port
/// from a byte-sum hash of the name, so the same name always maps to the
/// same port without any coordination between processes.
pub fn daemon_socket_addr(instance: Option<&str>) -> SocketAddr {
	let port = match instance {
		Some(name) => INSTANCE_PORT_BASE + (name.bytes().map(|b| b as u16).sum::<u16>() % 1000),
		None => DEFAULT_PORT,
	};
	SocketAddr::from((Ipv4Addr::LOCALHOST, port))
}

/// Resolve the data directory a daemon instance owns.
///
/// Named instances live under `instances/<name>` so they never touch the
/// default installation's state. Every process that addresses an instance —
/// the daemon, the CLI, the server, the native app — must apply this rule
/// identically, or a client reads a different device identity than the
/// daemon it is talking to.
pub fn instance_data_dir(base: PathBuf, instance: Option<&str>) -> PathBuf {
	match instance {
		Some(name) => base.join("instances").join(name),
		None => base,
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn instance_dirs_are_scoped() {
		let base = PathBuf::from("/data/spacedrive");
		assert_eq!(instance_data_dir(base.clone(), None), base);
		assert_eq!(
			instance_data_dir(base, Some("smbtest")),
			PathBuf::from("/data/spacedrive/instances/smbtest")
		);
	}

	#[test]
	fn instance_ports_are_stable_and_distinct() {
		assert_eq!(daemon_socket_addr(None).port(), 6969);
		let a = daemon_socket_addr(Some("alpha")).port();
		assert_eq!(a, daemon_socket_addr(Some("alpha")).port());
		assert_ne!(a, daemon_socket_addr(Some("beta")).port());
	}
}
