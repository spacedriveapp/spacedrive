pub struct ConnectionPool {
    connected_clients: Vec<Client>,
}

pub struct Client {
    tcp_address: String,
    remote_address: String,
    client_uuid: String,
}
