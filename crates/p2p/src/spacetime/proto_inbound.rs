use std::{future::Future, pin::Pin, sync::Arc};

use libp2p::{core::UpgradeInfo, swarm::NegotiatedSubstream, InboundUpgrade};

use crate::{Manager, ManagerStreamAction, Metadata, PeerId, PeerMessageEvent};

use super::{SpaceTimeProtocolName, SpaceTimeStream};

pub struct InboundProtocol<TMetadata: Metadata> {
    pub(crate) peer_id: PeerId,
    pub(crate) manager: Arc<Manager<TMetadata>>,
}

impl<TMetadata: Metadata> UpgradeInfo for InboundProtocol<TMetadata> {
    type Info = SpaceTimeProtocolName;
    type InfoIter = [Self::Info; 1];

    fn protocol_info(&self) -> Self::InfoIter {
        [SpaceTimeProtocolName(self.manager.application_name)]
    }
}

impl<TMetadata: Metadata> InboundUpgrade<NegotiatedSubstream> for InboundProtocol<TMetadata> {
    type Output = ();
    type Error = ();
    type Future = Pin<Box<dyn Future<Output = Result<Self::Output, Self::Error>> + Send + 'static>>;

    fn upgrade_inbound(self, io: NegotiatedSubstream, _: Self::Info) -> Self::Future {
        Box::pin(async move {
            Ok(self
                .manager
                .emit(ManagerStreamAction::Event(
                    PeerMessageEvent {
                        peer_id: self.peer_id,
                        manager: self.manager.clone(),
                        stream: SpaceTimeStream::new(io),
                        _priv: (),
                    }
                    .into(),
                ))
                .await)
        })
    }
}
