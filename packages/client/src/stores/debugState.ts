/// <reference types="vite/client" />
import { useSnapshot } from 'valtio';

import { valtioPersist } from '.';

export const debugState = valtioPersist('sd-debugState', {
	// @ts-ignore
	enabled: import.meta.env.DEV,
	rspcLogger: false
});

export function useDebugState() {
	return useSnapshot(debugState);
}

export function getDebugState() {
	return debugState;
}
