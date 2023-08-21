use sd_p2p::Component;

/// A component that hooks into the P2P system and reemits events to the frontend via an rspc subscription.
pub struct RspcComponent {}

impl RspcComponent {
	pub fn new() -> Self {
		Self {}
	}
}

impl Component for RspcComponent {
	// TODO
}
