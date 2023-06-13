import { useState } from 'react';
import { useBridgeSubscription } from '../rspc';

export function useOnlineLocations() {
	const [state, setState] = useState<number[][] | null>(null);

	// TODO: Move into a context with P2PEvents
	useBridgeSubscription(['locations.online'], {
		onData: (d) => setState(d)
	});

	return state;
}
