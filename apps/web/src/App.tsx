import { createWSClient, loggerLink, wsLink } from '@rspc/client';
import { PlatformProvider, hooks, queryClient } from '@sd/client';
import SpacedriveInterface, { Platform } from '@sd/interface';
import { useEffect } from 'react';

const wsClient = createWSClient({
	url: import.meta.env.VITE_SDSERVER_BASE_URL || 'ws://localhost:8080/rspc/ws'
});

const isDev = import.meta.env.DEV && false; // TODO: Remove false
const client = hooks.createClient({
	links: [
		...(isDev ? [loggerLink()] : []),
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
