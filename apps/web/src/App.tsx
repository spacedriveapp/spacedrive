import { hydrate, QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { useEffect, useRef } from 'react';
import { createBrowserRouter } from 'react-router-dom';
import { RspcProvider } from '@sd/client';
import { Platform, PlatformProvider, routes, SpacedriveInterface } from '@sd/interface';
import { useShowControls } from '@sd/interface/hooks';

import demoData from './demoData.json';
import ScreenshotWrapper from './ScreenshotWrapper';

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
	return `${currentURL.origin}/spacedrive`;
})();

const platform: Platform = {
	platform: 'web',
	getThumbnailUrlByThumbKey: (keyParts) =>
		`${spacedriveURL}/thumbnail/${keyParts.map((i) => encodeURIComponent(i)).join('/')}.webp`,
	getFileUrl: (libraryId, locationLocalId, filePathId) =>
		`${spacedriveURL}/file/${encodeURIComponent(libraryId)}/${encodeURIComponent(
			locationLocalId
		)}/${encodeURIComponent(filePathId)}`,
	getFileUrlByPath: (path) => `${spacedriveURL}/local-file-by-path/${encodeURIComponent(path)}`,
	openLink: (url) => window.open(url, '_blank')?.focus(),
	confirm: (message, cb) => cb(window.confirm(message)),
	auth: {
		start(url) {
			return window.open(url);
		},
		finish(win: Window | null) {
			win?.close();
		}
	}
};

const queryClient = new QueryClient({
	defaultOptions: {
		queries: {
			...(import.meta.env.VITE_SD_DEMO_MODE && {
				refetchOnWindowFocus: false,
				staleTime: Infinity,
				cacheTime: Infinity,
				networkMode: 'offlineFirst',
				enabled: false
			}),
			networkMode: 'always'
		},
		mutations: {
			networkMode: 'always'
		}
		// TODO: Mutations can't be globally disable which is annoying!
	}
});

const router = createBrowserRouter(routes);

function App() {
	const domEl = useRef<HTMLDivElement>(null);
	const { isEnabled: showControls } = useShowControls();

	useEffect(() => window.parent.postMessage('spacedrive-hello', '*'), []);

	if (import.meta.env.VITE_SD_DEMO_MODE === 'true') {
		hydrate(queryClient, demoData);
	}

	return (
		<ScreenshotWrapper showControls={!!showControls}>
			<div ref={domEl} className="App">
				<RspcProvider queryClient={queryClient}>
					<PlatformProvider platform={platform}>
						<QueryClientProvider client={queryClient}>
							<SpacedriveInterface router={router} />
						</QueryClientProvider>
					</PlatformProvider>
				</RspcProvider>
			</div>
		</ScreenshotWrapper>
	);
}

export default App;
