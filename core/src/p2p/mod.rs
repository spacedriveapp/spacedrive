use tokio::sync::mpsc;

pub mod listener;
pub mod pool;

pub struct PeerConnection {
  pub client_uuid: String,
  pub tcp_address: String,
  pub message_sender: mpsc::Sender<String>,
}
