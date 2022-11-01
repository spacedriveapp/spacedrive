import { createWSClient, loggerLink, splitLink, wsLink } from '@rspc/client';
import { getDebugState, hooks, queryClient } from '@sd/client';
import SpacedriveInterface, { Platform, PlatformProvider } from '@sd/interface';
import { useEffect } from 'react';

globalThis.isDev = import.meta.env.DEV;

const wsClient = createWSClient({
	url: import.meta.env.VITE_SDSERVER_BASE_URL || 'ws://localhost:8080/rspc/ws'
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

const platform: Platform = {
	platform: 'web',
	getThumbnailUrlById: (casId) => `spacedrive://thumbnail/${encodeURIComponent(casId)}`,
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
