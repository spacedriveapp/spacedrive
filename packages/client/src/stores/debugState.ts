import { useEffect, useState } from 'react';
import { createMutable } from 'solid-js/store';

import { createPersistedMutable, useSolidStore } from '../solid';

export interface DebugState {
	enabled: boolean;
	rspcLogger: boolean;
	reactQueryDevtools: 'enabled' | 'disabled' | 'invisible';
	shareFullTelemetry: boolean; // used for sending telemetry even if the app is in debug mode
	telemetryLogging: boolean;
}

export const debugState = createPersistedMutable(
	'sd-debugState',
	createMutable<DebugState>({
		enabled: globalThis.isDev,
		rspcLogger: false,
		reactQueryDevtools: globalThis.isDev ? 'invisible' : 'enabled',
		shareFullTelemetry: false,
		telemetryLogging: false
	})
);

export function useDebugState() {
	return useSolidStore(debugState);
}

export function useDebugStateEnabler(): () => void {
	const [clicked, setClicked] = useState(0);

	useEffect(() => {
		if (clicked >= 5) {
			debugState.enabled = true;
		}

		const timeout = setTimeout(() => setClicked(0), 1000);

		return () => clearTimeout(timeout);
	}, [clicked]);

	return () => setClicked((c) => c + 1);
}
