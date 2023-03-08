use serde::{Deserialize, Serialize};

/// TODO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SpaceTimeMessage {
    /// Establish the connection
    Establish,

    /// Send data on behalf of application
    Application(Vec<u8>),
}
