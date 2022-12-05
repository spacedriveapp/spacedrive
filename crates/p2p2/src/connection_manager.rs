use std::{future::Future, marker::PhantomData, net::SocketAddr, sync::Arc};

use futures_util::{AsyncReadExt, StreamExt};
use serde::{de::DeserializeOwned, Serialize};

use crate::{Connection, PeerId, State, Stream, Transport, TransportConnection};

pub struct ConnectionManager<T, TPayload, THandlerFn, THandlerFut>
where
    T: Transport,
    TPayload: Serialize + DeserializeOwned + Send + Sync + 'static,
    THandlerFn: Fn(TPayload, Stream<<T::Connection as TransportConnection>::Stream>) -> THandlerFut
        + Clone
        + Send
        + Sync
        + 'static,
    THandlerFut: Future<Output = ()> + Send + Sync + 'static,
{
    transport: T,
    endpoint_state: Arc<State>,
    state: T::State,
    phantom: PhantomData<(TPayload, THandlerFn, THandlerFut)>,
}

impl<T, TPayload, THandlerFn, THandlerFut> ConnectionManager<T, TPayload, THandlerFn, THandlerFut>
where
    T: Transport,
    TPayload: Serialize + DeserializeOwned + Send + Sync + 'static,
    THandlerFn: Fn(TPayload, Stream<<T::Connection as TransportConnection>::Stream>) -> THandlerFut
        + Clone
        + Send
        + Sync
        + 'static,
    THandlerFut: Future<Output = ()> + Send + Sync + 'static,
{
    pub fn new(
        mut transport: T,
        endpoint_state: Arc<State>,
        handler_fn: THandlerFn,
    ) -> (Arc<Self>, SocketAddr) {
        let (stream, state) = transport.listen(endpoint_state.clone());
        let listen_addr = transport.listen_addr(state.clone());
        let this = Arc::new(Self {
            transport,
            endpoint_state,
            state,
            phantom: PhantomData,
        });

        // TODO: Invert it so the user spawns this for us???? -> Could make runtime agnostic?
        tokio::spawn(this.clone().event_loop(handler_fn.clone(), stream));

        (this, listen_addr)
    }

    // TODO: Drop thread when `ConnectionManager` is dropped
    async fn event_loop(self: Arc<Self>, handler_fn: THandlerFn, mut stream: T::ListenStream) {
        loop {
            while let Some(conn) = stream.next().await {
                tokio::spawn(self.clone().handle_connection(handler_fn.clone(), conn));
            }
        }
    }

    // TODO: Drop thread when `ConnectionManager` is dropped
    async fn handle_connection(self: Arc<Self>, handler_fn: THandlerFn, conn: T::ListenStreamItem) {
        let mut conn = self
            .transport
            .accept(self.state.clone(), conn.await.unwrap());
        let mut stream = conn.listen();
        let conn = Arc::new(conn);

        while let Some(stream) = stream.next().await {
            let stream = conn.accept_stream(stream.unwrap());
            let peer_id = conn.peer_id().unwrap();

            // TODO: Reenable this
            // if let Some(server_name) = handshake_data.server_name {
            // 	if server_name != peer_id.to_string() {
            // 		println!("{} {}", server_name, peer_id.to_string()); // TODO: BRUH
            // 		println!(
            // 			"p2p warning: client presented a certificate and servername which don't match!"
            // 		);
            // 		return;
            // 	}
            // } else {
            // 	println!(
            // 		"p2p warning: client presented a certificate and servername which don't match!"
            // 	);
            // 	return;
            // }

            // // TODO: Do this check again before adding to array because the `ConnectionEstablishmentPayload` adds delay
            // if self.is_peer_connected(&peer_id) && self.peer_id > peer_id {
            //     debug!(
            //         "Closing new connection to peer '{}' as we are already connect!",
            //         peer_id
            //     );
            //     connection.close(VarInt::from_u32(0), b"DUP_CONN");
            //     return;
            // }

            if !self.endpoint_state.on_incoming_connection(&peer_id) {
                todo!(); // TODO
            }

            tokio::spawn(self.clone().handle_stream(
                handler_fn.clone(),
                peer_id,
                conn.clone(),
                stream,
            ));
        }
    }

    // TODO: Drop thread when `ConnectionManager` is dropped
    async fn handle_stream(
        self: Arc<Self>,
        handler_fn: THandlerFn,
        peer_id: PeerId,
        conn: Arc<T::Connection>,
        mut stream: <T::Connection as TransportConnection>::Stream,
    ) {
        // TODO: Fire off preconnect behavour hook
        if !self.endpoint_state.on_incoming_stream(&peer_id) {
            todo!(); // TODO
        }

        // TODO: Read initial message and ensure it is sent within reasonable timeout
        // let stream = tokio::select! {
        //     stream = bi_streams.next() => {
        //         match stream {
        //             Some(stream) => stream,
        //             None => {
        //                 warn!("connection closed before we could read from it!");
        //                 return;
        //             }
        //         }
        //     }
        //     _ = sleep(Duration::from_secs(1)) => {
        //         warn!("Connection create connection establishment stream in expected time.");
        //         return;
        //     }

        // };

        // TODO: Timeout reading this message
        // TODO: Make sure there is a limit on how much can be read for this initial msg.
        // TODO: Error handling

        let mut output = [0u8; 100]; // TODO: What should this value be because it leaks to userspace with their `TPayload`
        let bytes = stream.read(&mut output).await.unwrap(); // TODO: Max size
        let payload = rmp_serde::from_slice(&output).unwrap();

        // TODO: Add connection into `active_conn` map

        // We pass off control of this stream to the root application
        handler_fn(payload, Stream::new(stream)).await;
    }

    /// TODO
    // TODO: Accept `PeerId`, `SocketAddr`, `Vec<SocketAddr>`, etc -> Dealing with priority of SocketAddrs???
    pub async fn connect(
        &self,
        socket_addr: SocketAddr,
    ) -> Result<Connection<TPayload, T::Connection>, T::EstablishError> {
        Ok(Connection::new(
            self.transport.accept(
                self.state.clone(),
                self.transport
                    .establish(self.state.clone(), socket_addr)?
                    .await
                    .unwrap(), // TODO: Error handling
            ),
        ))
    }

    /// TODO
    pub(crate) fn close(&self) {
        // self.end
        todo!(); // TODO: Pass off to transport
    }
}
