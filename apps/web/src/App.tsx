import { WebsocketTransport, createClient } from '@rspc/client';
import { Operations, PlatformProvider, queryClient, rspc } from '@sd/client';
import SpacedriveInterface, { Platform } from '@sd/interface';
import { useEffect } from 'react';

const client = createClient<Operations>({
	transport: new WebsocketTransport(
		import.meta.env.VITE_SDSERVER_BASE_URL || 'ws://localhost:8080/rspcws'
	)
});

// TODO: Conditional 'demoMode'

const platform: Platform = {
	platform: 'web',
	getThumbnailUrlById: (casId) => `spacedrive://thumbnail/${encodeURIComponent(casId)}`,
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
