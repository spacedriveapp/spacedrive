import { useState } from 'react';
import { useBridgeSubscription } from '../rspc';

export function useOnlineLocations() {
	const [state, setState] = useState<number[][] | null>(null);

	useBridgeSubscription(['locations.online'], {
		onData: (d) => setState(d)
	});

	return state;
}
