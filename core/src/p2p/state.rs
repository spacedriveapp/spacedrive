use std::collections::HashMap;

use uuid::Uuid;

pub struct State {
	libraries: HashMap<Uuid, ()>,
}

impl State {
	// TODO: Subscribe to updates

	// TODO: Into mDNS service
}
