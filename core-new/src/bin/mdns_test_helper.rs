//! Minimal LibP2P mDNS test helper - just tests peer discovery
//! Usage: mdns_test_helper listen|discover

use libp2p::{
    identify,
    mdns,
    swarm::{NetworkBehaviour, SwarmEvent, Config as SwarmConfig},
    PeerId, Swarm, Transport,
    tcp, noise, yamux, core::upgrade,
    futures::StreamExt,
};
use std::error::Error;
use std::time::Duration;
use tokio::time::timeout;

#[derive(NetworkBehaviour)]
struct Behaviour {
    identify: identify::Behaviour,
    mdns: mdns::tokio::Behaviour,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} listen|discover", args[0]);
        std::process::exit(1);
    }

    let mode = &args[1];
    
    // Create a random PeerId
    let local_key = libp2p::identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    println!("🆔 Local peer ID: {}", local_peer_id);

    // Set up the transport
    let transport = tcp::tokio::Transport::default()
        .upgrade(upgrade::Version::V1)
        .authenticate(noise::Config::new(&local_key)?)
        .multiplex(yamux::Config::default())
        .boxed();

    // Create the network behaviour
    let behaviour = Behaviour {
        identify: identify::Behaviour::new(identify::Config::new(
            "/test/1.0.0".to_string(),
            local_key.public(),
        )),
        mdns: mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id)?,
    };

    // Create the swarm
    let mut swarm = Swarm::new(transport, behaviour, local_peer_id, SwarmConfig::with_tokio_executor());

    // Listen on a random local port
    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    match mode.as_str() {
        "listen" => run_listener(swarm).await,
        "discover" => run_discoverer(swarm).await,
        _ => {
            eprintln!("Invalid mode: {}. Use 'listen' or 'discover'", mode);
            std::process::exit(1);
        }
    }
}

async fn run_listener(mut swarm: Swarm<Behaviour>) -> Result<(), Box<dyn Error>> {
    println!("👂 Starting mDNS listener...");
    
    loop {
        match swarm.select_next_some().await {
            SwarmEvent::NewListenAddr { address, .. } => {
                println!("📡 Listening on: {}", address);
            }
            SwarmEvent::Behaviour(event) => match event {
                BehaviourEvent::Mdns(mdns::Event::Discovered(list)) => {
                    for (peer_id, multiaddr) in list {
                        println!("🔍 Discovered peer: {} at {}", peer_id, multiaddr);
                        println!("PEER_DISCOVERED:{}", peer_id);
                    }
                }
                BehaviourEvent::Mdns(mdns::Event::Expired(list)) => {
                    for (peer_id, multiaddr) in list {
                        println!("⏰ Peer expired: {} at {}", peer_id, multiaddr);
                    }
                }
                BehaviourEvent::Identify(identify::Event::Received {
                    peer_id,
                    info: identify::Info { listen_addrs, .. },
                    ..
                }) => {
                    println!("🆔 Identified peer: {} with addresses: {:?}", peer_id, listen_addrs);
                }
                _ => {}
            },
            _ => {}
        }
    }
}

async fn run_discoverer(mut swarm: Swarm<Behaviour>) -> Result<(), Box<dyn Error>> {
    println!("🔍 Starting mDNS discoverer (10 second timeout)...");
    
    let discovery_result = timeout(Duration::from_secs(10), async {
        loop {
            match swarm.select_next_some().await {
                SwarmEvent::NewListenAddr { address, .. } => {
                    println!("📡 Discoverer listening on: {}", address);
                }
                SwarmEvent::Behaviour(event) => match event {
                    BehaviourEvent::Mdns(mdns::Event::Discovered(list)) => {
                        for (peer_id, multiaddr) in list {
                            println!("✅ FOUND PEER: {} at {}", peer_id, multiaddr);
                            println!("PEER_DISCOVERED:{}", peer_id);
                            return Ok::<(), Box<dyn Error>>(());
                        }
                    }
                    BehaviourEvent::Identify(identify::Event::Received {
                        peer_id,
                        info: identify::Info { listen_addrs, .. },
                        ..
                    }) => {
                        println!("🆔 Identified peer: {} with addresses: {:?}", peer_id, listen_addrs);
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }).await;

    match discovery_result {
        Ok(_) => {
            println!("🎉 Discovery successful!");
            Ok(())
        }
        Err(_) => {
            println!("⏰ Discovery timed out after 10 seconds");
            Err("Discovery timeout".into())
        }
    }
}