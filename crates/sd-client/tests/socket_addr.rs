use std::net::{Ipv4Addr, SocketAddr};

use sd_client::daemon_socket_addr;

#[test]
fn default_instance_uses_port_6969() {
	assert_eq!(
		daemon_socket_addr(None),
		SocketAddr::from((Ipv4Addr::LOCALHOST, 6969))
	);
}

#[test]
fn named_instances_match_the_byte_sum_derivation() {
	// Expected ports mirror the original inline derivation:
	// 6970 + (sum of the name's bytes % 1000)
	for (name, port) in [
		("work", 7421),
		("alpha", 7488),
		("a", 7067),
		("test-instance_2", 7461),
	] {
		assert_eq!(
			daemon_socket_addr(Some(name)),
			SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
			"instance {name:?}"
		);
	}
}

#[test]
fn derivation_is_stable_and_stays_in_the_instance_range() {
	for name in ["work", "home", "ci", "some_longer_instance-name"] {
		let addr = daemon_socket_addr(Some(name));
		assert_eq!(addr, daemon_socket_addr(Some(name)));
		assert!((6970..7970).contains(&addr.port()));
	}
}

#[test]
fn address_renders_like_the_original_string_format() {
	assert_eq!(daemon_socket_addr(None).to_string(), "127.0.0.1:6969");
	assert_eq!(
		daemon_socket_addr(Some("work")).to_string(),
		"127.0.0.1:7421"
	);
}
