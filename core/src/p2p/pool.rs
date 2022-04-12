use crate::client::Client;

pub struct ClientPool {
  pub clients: Vec<Client>,
}
