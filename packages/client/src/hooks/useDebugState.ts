import { useEffect, useState } from 'react';
import { useSnapshot } from 'valtio';
import { valtioPersist } from '../lib/valito';

export interface DebugState {
	enabled: boolean;
	rspcLogger: boolean;
	reactQueryDevtools: 'enabled' | 'disabled' | 'invisible';
	shareFullTelemetry: boolean; // used for sending telemetry even if the app is in debug mode
	telemetryLogging: boolean;
}

const debugState: DebugState = valtioPersist('sd-debugState', {
	enabled: globalThis.isDev,
	rspcLogger: false,
	reactQueryDevtools: globalThis.isDev ? 'invisible' : 'enabled',
	shareFullTelemetry: false,
	telemetryLogging: false
});

export function useDebugState() {
	return useSnapshot(debugState);
}

export function getDebugState() {
	return debugState;
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
