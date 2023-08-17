use crate::Metadata;

// TODO: Move into it's own file
/// TODO: Debug + Clone
pub struct Service<T: Metadata> {
	// discovery_tx:
	name: String,
	metadata: T,
}

impl<T: Metadata> Service<T> {
	// pub fn get();

	// pub fn update() {
	// 	self.discovery_tx.send(()).unwrap();
	// }
}
