import { WebsocketTransport, createClient } from '@rspc/client';
import { PlatformProvider, Procedures, queryClient, rspc } from '@sd/client';
import SpacedriveInterface, { Platform } from '@sd/interface';
import { useEffect } from 'react';

const client = createClient<Procedures>({
	transport: new WebsocketTransport(
		import.meta.env.VITE_SDSERVER_BASE_URL || 'ws://localhost:8080/rspc/ws'
	)
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
			<rspc.Provider client={client} queryClient={queryClient}>
				<PlatformProvider platform={platform}>
					<SpacedriveInterface />
				</PlatformProvider>
			</rspc.Provider>
		</div>
	);
}

export default App;
