import { createWSClient, loggerLink, wsLink } from '@rspc/client';
import { QueryClient, QueryClientProvider, hydrate } from '@tanstack/react-query';
import { useEffect } from 'react';
import { getDebugState, hooks } from '@sd/client';
import SpacedriveInterface, { Platform, PlatformProvider } from '@sd/interface';
import demoData from './demoData.json';

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
	demoMode: import.meta.env.VITE_SD_DEMO_MODE === 'true'
};

const queryClient = new QueryClient({
	defaultOptions: {
		queries: import.meta.env.VITE_SD_DEMO_MODE
			? {
					refetchOnWindowFocus: false,
					staleTime: Infinity,
					cacheTime: Infinity,
					networkMode: 'offlineFirst',
					enabled: false
			  }
			: undefined
		// TODO: Mutations can't be globally disable which is annoying!
	}
});

function App() {
	useEffect(() => window.parent.postMessage('spacedrive-hello', '*'), []);

	if (import.meta.env.VITE_SD_DEMO_MODE === 'true') {
		hydrate(queryClient, demoData);
	}

	return (
		<div className="App">
			<hooks.Provider client={client} queryClient={queryClient}>
				<PlatformProvider platform={platform}>
					<QueryClientProvider client={queryClient}>
						<SpacedriveInterface router="browser" />
					</QueryClientProvider>
				</PlatformProvider>
			</hooks.Provider>
		</div>
	);
}

export default App;
