import { QueryClient, QueryClientProvider, hydrate } from '@tanstack/react-query';
import { useEffect } from 'react';
import { createBrowserRouter } from 'react-router-dom';
import { RspcProvider } from '@sd/client';
import { Platform, PlatformProvider, SpacedriveInterface, routes } from '@sd/interface';
import demoData from './demoData.json';

// TODO: Restore this once TS is back up to functionality in rspc.
// const wsClient = createWSClient({
// 	url: `ws://${serverOrigin}/rspc/ws`
// });

// const client = hooks.createClient({
// 	links: [
// 		loggerLink({
// 			enabled: () => getDebugState().rspcLogger
// 		}),
// 		wsLink({
// 			client: wsClient
// 		})
// 	]
// });

const spacedriveURL = (() => {
	const currentURL = new URL(window.location.href);
	if (import.meta.env.VITE_SDSERVER_ORIGIN) {
		currentURL.host = import.meta.env.VITE_SDSERVER_ORIGIN;
	} else if (import.meta.env.DEV) {
		currentURL.host = 'localhost:8080';
	}
	currentURL.pathname = 'spacedrive';
	return currentURL.href;
})();

const platform: Platform = {
	platform: 'web',
	getThumbnailUrlByThumbKey: (keyParts) =>
		`${spacedriveURL}/thumbnail/${keyParts.map((i) => encodeURIComponent(i)).join('/')}.webp`,
	getFileUrl: (libraryId, locationLocalId, filePathId) =>
		`${spacedriveURL}/file/${encodeURIComponent(libraryId)}/${encodeURIComponent(
			locationLocalId
		)}/${encodeURIComponent(filePathId)}`,
	openLink: (url) => window.open(url, '_blank')?.focus(),
	confirm: (message, cb) => cb(window.confirm(message))
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

const router = createBrowserRouter(routes);

function App() {
	useEffect(() => window.parent.postMessage('spacedrive-hello', '*'), []);

	if (import.meta.env.VITE_SD_DEMO_MODE === 'true') {
		hydrate(queryClient, demoData);
	}

	return (
		<div className="App">
			<RspcProvider queryClient={queryClient}>
				<PlatformProvider platform={platform}>
					<QueryClientProvider client={queryClient}>
						<SpacedriveInterface router={router} />
					</QueryClientProvider>
				</PlatformProvider>
			</RspcProvider>
		</div>
	);
}

export default App;
