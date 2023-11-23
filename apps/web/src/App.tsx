import { hydrate, QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { useEffect, useRef, useState } from 'react';
import { createBrowserRouter } from 'react-router-dom';
import { RspcProvider } from '@sd/client';
import { Platform, PlatformProvider, routes, SpacedriveInterface } from '@sd/interface';
import { useOperatingSystem, useShowControls } from '@sd/interface/hooks';

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
	},
	landingApiOrigin: 'https://spacedrive.com'
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

function App() {
	const os = useOperatingSystem();
	const [router, setRouter] = useState(() => {
		const router = createBrowserRouter(routes(os));

		router.subscribe((event) => {
			setRouter((router) => {
				const currentIndex: number | undefined = history.state?.idx;
				if (currentIndex === undefined) return router;

				return {
					...router,
					currentIndex,
					maxIndex:
						event.historyAction === 'PUSH'
							? currentIndex
							: // sometimes the max index is 0 when the current index is > 0, like when reloading the page -_-
							  Math.max(router.maxIndex, currentIndex)
				};
			});
		});

		return {
			router,
			currentIndex: 0,
			maxIndex: 0
		};
	});

	const domEl = useRef<HTMLDivElement>(null);
	const { isEnabled: showControls } = useShowControls();

	useEffect(() => window.parent.postMessage('spacedrive-hello', '*'), []);

	if (
		import.meta.env.VITE_SD_DEMO_MODE === 'true' &&
		// quick and dirty check for if we've already rendered lol
		domEl === null
	) {
		hydrate(queryClient, demoData);
	}

	return (
		<ScreenshotWrapper showControls={!!showControls}>
			<div ref={domEl} className="App">
				<RspcProvider queryClient={queryClient}>
					<PlatformProvider platform={platform}>
						<QueryClientProvider client={queryClient}>
							<SpacedriveInterface
								routing={{
									...router,
									routerKey: 0
								}}
							/>
						</QueryClientProvider>
					</PlatformProvider>
				</RspcProvider>
			</div>
		</ScreenshotWrapper>
	);
}

export default App;
