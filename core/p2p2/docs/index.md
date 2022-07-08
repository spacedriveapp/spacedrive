# Spacedrive Peer to Peer

This document outlines the peer to peer protocol used by the Spacedrive desktop application. This document is designed to outlined how the system works and also discuss the security and decision making behind the system.

## Concepts

 - **Peer** - TODO

### P2PManager

The peer to peer library is designed to be general purpose. This means none of the Spacedrive code is directly tied into the peer to peer library. This makes for a very nice separation of concerns but also introduces the requirement for an abstraction to sit above the P2P library so it can properly make decisions with the data that Spacedrive holds. This is where the `P2PManager` trait comes in. You must implement the `P2PManager` trait in your application code and then the peer to peer system will call various hooks, allowing your system to react to various events.

The `P2PManager` is implemented as a Rust trait which works very well for allowing the application to hook into the peer to peer system, however, in Rust async functions are not properly supported in traits. This is works very well when combined with an `tokio::mpsc::unbound_channel()` implemented in your application, so that you can run async code in response to a specific situtation. The only expect to syncronus methods is the `peer_paired` method as we want to be sure that the peer was properly saved into the database on both sides. This method returns a `Pin<Box<dyn Future<Output = Result<(), ()>>>>`.

It's important we maintain a good separation between the `P2PManager` and the application which is using it. This led to to the decision to make the P2P system focusing on getting a stream of bytes (`Vec<u8>`) between peers. This means the P2P system does not enforce a specific serialization method for your data, this gives you the choice in your application layer to choice whatever works best for the type of data you are going to be sending. This also means you are responsible for data compression. We use the [`rmp_serde`](https://crates.io/crates/rmp-serde) crate (which uses [msgpack](https://msgpack.org/index.html)) internally to send data between clients, and would reccomend it in your application however, this decision is entirely up to you.

### Identity keypair

Unpon installing Spacedrive your application with generate a public and private key paired which is called the identity keypair. These certificates facilitate secure communication and identify the client to other nodes.

### Peer ID's

Each peer has a unique identifier which is called a peer id. This identifier is derived from a [SHA-1](https://en.wikipedia.org/wiki/SHA-1) hash of the identity keypair's public key.

**Note: We might change from SHA-1 to SHA-255 or SHA-256 in the near future. The current limitation is the maximum size of a DNS TXT record. No known SHA-1 collisions exist for certificates but given SHA-1 has been broken it would be preferable to use something more secure.**

## Discovery

Discovery is the first phase of the peer to peer process. The goal of discovery is to determine a list of other peers which we could potentially pair to. This phase is made up of multiple different protocol and the result of all of the systems are combined and returned to the application.

### LAN

To discovery other machines running Spacedrive over your local network, we make use of [mDNS](). mDNS is a protocol which transmits DNS packets using multicast UDP which allows a DNS record to be published to your local network and read by other devices on the network. This system is used commonly by other systems with similar goals such as [libp2p]() and [Apple's Airplay](). We are using [DNS-SD]() which makes use of DNS SRV and TXT records to advertise information about the current peer.

Spacedrive advertise a SRV record that looks like:

_{peer_id}_spacedrive_._udp_.local. 86400 IN SRV 10 5 5223 server.example.com.

This system will continue to passively discover clients while Spacedrive is running.

### Global Discovery

The global discovery system works in a different way. TODO

#### Announcement

TODO: Discuss proto + security

```rust
Message::ClientAnnouncement { peer_id, addresses: vec!["192.168.0.1".to_string(), "1.1.1.1".to_string()] }
```

#### Query

TODO

```rust
Message::QueryClientAnnouncement(vec![peer_id, peer_id2]);
```











## General Overview

This system is designed on top of the following main technologies:
 - [QUIC]() - A tcp-like protocol built on top of UDP. QUIC also supports [TLS 1.3]() for encryption and pro





## Pairing

TODO

# External Resources

 - TODO: Magic Wormhole talk
 - TODO: Syncthing spec
