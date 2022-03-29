use futures::StreamExt;
use libp2p::{
	identity, ping,
	swarm::{Swarm, SwarmEvent},
	Multiaddr, PeerId,
};
use std::error::Error;

pub async fn listen(port: Option<u32>) -> Result<(), Box<dyn Error>> {
	let local_key = identity::Keypair::generate_ed25519();
	let local_peer_id = PeerId::from(local_key.public());
	println!("Local peer id: {:?}", local_peer_id);

	let transport = libp2p::development_transport(local_key).await?;

	// Create a ping network behavior.
	//
	// For illustrative purposes, the ping protocol is configured to
	// keep the connection alive, so a continuous sequence of pings
	// can be observed.
	let behavior = ping::Behaviour::new(ping::Config::new().with_keep_alive(true));

	let mut swarm = Swarm::new(transport, behavior, local_peer_id);

	// Tell the swarm to listen on all interfaces and a random, OS-assigned
	// port.
	swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

	// Dial the peer identified by the multi-address given as the second
	// command-line argument, if any.

	if port.is_some() {
		let addr = format!("{:?}{:?}", "/ip4/127.0.0.1/tcp/", port);
		let remote: Multiaddr = addr.parse()?;
		swarm.dial(remote)?;
		println!("Dialed {}", addr)
	}

	loop {
		match swarm.select_next_some().await {
			SwarmEvent::NewListenAddr { address, .. } => {
				println!("Listening on {:?}", address)
			},
			SwarmEvent::Behaviour(event) => println!("{:?}", event),
			_ => {},
		}
	}
}
