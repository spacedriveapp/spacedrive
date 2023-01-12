import { createWSClient, loggerLink, wsLink } from '@rspc/client';
import { getDebugState, hooks, queryClient } from '@sd/client';
import SpacedriveInterface, { Platform, PlatformProvider } from '@sd/interface';
import { useEffect } from 'react';

globalThis.isDev = import.meta.env.DEV;

const serverOrigin = import.meta.env.VITE_SDSERVER_ORIGIN || 'localhost:8080';

const wsClient = createWSClient({
	url: `ws://${serverOrigin}/rspc/ws`
});

const client = hooks.createClient({
	links: [
		loggerLink({
			enabled: () => getDebugState().rspcLogger
		}),
		wsLink({
			client: wsClient
		})
	]
});

const http = isDev ? 'http' : 'https';

const platform: Platform = {
	platform: 'web',
	getThumbnailUrlById: (casId) =>
		`${http}://${serverOrigin}/spacedrive/thumbnail/${encodeURIComponent(casId)}.webp`,
	openLink: (url) => window.open(url, '_blank')?.focus(),
	demoMode: true
};

function App() {
	useEffect(() => window.parent.postMessage('spacedrive-hello', '*'), []);

	return (
		<div className="App">
			<hooks.Provider client={client} queryClient={queryClient}>
				<PlatformProvider platform={platform}>
					<SpacedriveInterface />
				</PlatformProvider>
			</hooks.Provider>
		</div>
	);
}

export default App;
