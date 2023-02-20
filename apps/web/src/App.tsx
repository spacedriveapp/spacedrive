import { createWSClient, loggerLink, wsLink } from '@rspc/client';
import { useEffect } from 'react';
import { getDebugState, hooks, queryClient } from '@sd/client';
import SpacedriveInterface, { Platform, PlatformProvider } from '@sd/interface';

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
const spacedriveProtocol = `${http}://${serverOrigin}/spacedrive`;

const platform: Platform = {
	platform: 'web',
	getThumbnailUrlById: (casId) =>
		`${spacedriveProtocol}/thumbnail/${encodeURIComponent(casId)}.webp`,
	getFileUrl: (libraryId, locationLocalId, filePathId) =>
		`${spacedriveProtocol}/file/${encodeURIComponent(libraryId)}/${encodeURIComponent(
			locationLocalId
		)}/${encodeURIComponent(filePathId)}`,
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
