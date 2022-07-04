// pub const fn to_byte_identifier(&self) -> u8 {
// 	match self {
// 		Message::ClientAnnouncement { .. } => 0x01,
// 		Message::ClientAnnouncementResponse => 0x02,
// 		Message::QueryClientAnnouncement(_) => 0x03,
// 		Message::QueryClientAnnouncementResponse { .. } => 0x04,
// 	}
// }

// pub fn write<W>(self, wr: &mut W) -> Result<(), io::Error>
// where
// 	W: Write,
// {
// 	// write_ext_meta(wr, 1, SPACEDRIVE_TUNNEL_MSG_TYPE_MSGPACK_EXT)?;
// 	// wr.write_all(&[self.to_byte_identifier()])?;

// 	// match self {
// 	// 	Message::ClientAnnouncement { peer_id, addresses } => {
// 	// 		// write_map_len(wr, 2)?;

// 	// 		// write_str(wr, "peer_id")?;
// 	// 		// write_str(wr, &peer_id)?;

// 	// 		// write_str(wr, "addresses")?;
// 	// 		// write_str(wr, "peer_id")?;

// 	// 		// write_map_len(wr, 2)?;
// 	// 		// write_str(wr, &peer_id)?;
// 	// 		// write_array_len(wr, addresses.len() as u32)?;
// 	// 		// for addr in addresses {
// 	// 		// 	write_str(wr, &addr)?;
// 	// 		// }
// 	// 	},
// 	// 	Message::ClientAnnouncementResponse => {},
// 	// 	Message::QueryClientAnnouncement(_) => {
// 	// 		// write_map_len(wr, 2)?;
// 	// 		// write_str(wr, &peer_id)?;
// 	// 		// write_array_len(wr, addresses.len() as u32)?;
// 	// 		// for addr in addresses {
// 	// 		// 	write_str(wr, &addr)?;
// 	// 		// }
// 	// 	},
// 	// 	Message::QueryClientAnnouncementResponse { .. } => {},
// 	// }

// 	Ok(())
// }

// pub fn read<R>(rd: &mut R) -> Result<Self, ()>
// where
// 	R: Read,
// {
// 	let ext_meta = read_ext_meta(rd).unwrap();
// 	if ext_meta.typeid != SPACEDRIVE_MSG_TYPE_MSGPACK_EXT {
// 		return Err(());
// 	} else if ext_meta.size != 1 {
// 		return Err(());
// 	}
// 	let byte_identifier = {
// 		let mut buf = [0; 1];
// 		rd.read_exact(&mut buf).unwrap();
// 		buf[0]
// 	};

// 	match byte_identifier {
//         0x01 /* Message::Ping */ => Ok(Message::Ping),
//         0x02 /* Message::Pong */ => Ok(Message::Pong),
//         0x03 /* Message::File */ => {
//             unimplemented!(); // TODO: Allow reading files back from message.
//         },
//         0x04 /* Message::SyncPayload */ => {
//             let len = read_bin_len(rd).unwrap();
//             let mut buf = Vec::with_capacity(len as usize);
//             rd.take(len as u64).read_to_end(&mut buf).unwrap();
//             Ok(Message::SyncPayload(rmp_serde::decode::from_slice(&buf).unwrap()))
//         }
//         _ => Err(()),
//     }
// }
