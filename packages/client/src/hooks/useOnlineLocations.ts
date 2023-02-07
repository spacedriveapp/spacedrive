import { useState } from 'react';
import { useBridgeSubscription } from '../rspc';

export function useOnlineLocations() {
	const [state, setState] = useState<number[][] | null>(null);

	// @ts-expect-error
	useBridgeSubscription(['locations.online', null], {
		onData: (d) => setState(d)
	});

	return state;
}
