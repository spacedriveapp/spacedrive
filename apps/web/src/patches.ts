import { wsBatchLink } from '@rspc/client/v2';

const serverOrigin = import.meta.env.VITE_SDSERVER_ORIGIN || 'localhost:8080';

globalThis.isDev = import.meta.env.DEV;
globalThis.rspcLinks = [
	// TODO
	// loggerLink({
	// 	enabled: () => getDebugState().rspcLogger
	// }),
	wsBatchLink({
		url: `ws://${serverOrigin}/rspc/ws`
	})
];
