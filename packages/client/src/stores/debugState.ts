import { useSnapshot } from 'valtio';

import { valtioPersist } from './util';

export const debugState = valtioPersist('sd-debugState', {
	enabled: globalThis.isDev,
	rspcLogger: false,
	reactQueryDevtools: (globalThis.isDev ? 'invisible' : 'enabled') as
		| 'enabled'
		| 'disabled'
		| 'invisible'
});

export function useDebugState() {
	return useSnapshot(debugState);
}

export function getDebugState() {
	return debugState;
}
