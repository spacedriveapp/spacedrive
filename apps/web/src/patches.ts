import { wsBatchLink } from '@spacedrive/rspc-client';

globalThis.isDev = import.meta.env.DEV;
globalThis.rspcLinks = [
	// TODO
	// loggerLink({
	// 	enabled: () => getDebugState().rspcLogger
	// }),
	wsBatchLink({
		url: (() => {
			const currentURL = new URL(window.location.href);
			currentURL.protocol = currentURL.protocol === 'https:' ? 'wss:' : 'ws:';
			if (import.meta.env.VITE_SDSERVER_ORIGIN) {
				currentURL.host = import.meta.env.VITE_SDSERVER_ORIGIN;
			} else if (import.meta.env.DEV) {
				currentURL.host = 'localhost:8080';
			}
			currentURL.pathname = 'rspc/ws';
			return currentURL.href;
		})()
	})
];
