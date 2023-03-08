use std::future::{ready, Ready};

use libp2p::{core::UpgradeInfo, swarm::NegotiatedSubstream, OutboundUpgrade};
use tokio::{io::AsyncWriteExt, sync::oneshot};
use tracing::error;

use super::{SpaceTimeProtocolName, SpaceTimeStream};

#[derive(Debug)]
pub enum OutboundRequest {
    Data(Vec<u8>),
    Stream(oneshot::Sender<SpaceTimeStream>),
}

pub struct OutboundProtocol(pub(crate) &'static [u8], pub(crate) OutboundRequest);

impl UpgradeInfo for OutboundProtocol {
    type Info = SpaceTimeProtocolName;
    type InfoIter = [Self::Info; 1];

    fn protocol_info(&self) -> Self::InfoIter {
        [SpaceTimeProtocolName(self.0)]
    }
}

impl OutboundUpgrade<NegotiatedSubstream> for OutboundProtocol {
    type Output = ();
    type Error = ();
    type Future = Ready<Result<(), ()>>;

    fn upgrade_outbound(self, io: NegotiatedSubstream, _protocol: Self::Info) -> Self::Future {
        let mut stream = SpaceTimeStream::new(io);
        match self.1 {
            OutboundRequest::Data(data) => {
                tokio::spawn(async move {
                    if let Err(err) = stream.write_all(&data).await {
                        // TODO: Print the peer which we failed to send to here
                        error!("Error sending broadcast: {:?}", err);
                    }
                    stream.flush().await.unwrap();
                    stream.close().await.unwrap();
                    // TODO: We close the connection here without waiting for a response.
                    // TODO: If the other side's user-code doesn't account for that on this specific message they will error.
                    // TODO: Add an abstraction so the user can't respond to fixed size messages.
                });
            }
            OutboundRequest::Stream(sender) => {
                sender.send(stream).unwrap();
            }
        }

        ready(Ok(()))
    }
}
