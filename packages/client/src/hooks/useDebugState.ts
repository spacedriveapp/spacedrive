import { useEffect, useState } from 'react';
import { createMutable } from 'solid-js/store';
import { useSnapshot } from 'valtio';

import { valtioPersist } from '../lib/valito';
import { createPersistedMutable, useSolidStore } from '../solidjs-interop';

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

export function useDebugState2() {
	// TODO: Valtio would smartly track
	return useSolidStore(debugState);
}

export function useDebugState() {
	return useSnapshot(debugState2);
}

export function getDebugState() {
	return debugState2;
}

export function useDebugStateEnabler(): () => void {
	const [clicked, setClicked] = useState(0);

	useEffect(() => {
		if (clicked >= 5) {
			getDebugState().enabled = true;
		}

		const timeout = setTimeout(() => setClicked(0), 1000);

		return () => clearTimeout(timeout);
	}, [clicked]);

	return () => setClicked((c) => c + 1);
}

// TODO: Remove
window.demo = () => {
	console.log('Updated');
	debugState.enabled = !debugState.enabled;
};

// TODO: Remove
const debugState2: DebugState = valtioPersist('sd-debugState', {
	enabled: globalThis.isDev,
	rspcLogger: false,
	reactQueryDevtools: globalThis.isDev ? 'invisible' : 'enabled',
	shareFullTelemetry: false,
	telemetryLogging: false
});
