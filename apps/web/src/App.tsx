import { QueryClient, QueryClientProvider, hydrate } from '@tanstack/react-query';
import * as htmlToImage from 'html-to-image';
import { useEffect, useRef } from 'react';
import { createBrowserRouter } from 'react-router-dom';
import { RspcProvider } from '@sd/client';
import { Platform, PlatformProvider, routes, SpacedriveInterface } from '@sd/interface';

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
	const showControls = window.location.search.includes('showControls');

	const downloadImage = async () => {
		// Define a CSS rule to hide scrollbars
		const style = document.createElement('style');
		style.innerHTML = `
			::-webkit-scrollbar {
				display: none;
		  	}
			body, .no-scrollbar, .custom-scroll {
				overflow: hidden !important;
				-ms-overflow-style: none;  /* Internet Explorer 10+ */
				scrollbar-width: none;  /* Firefox */
			}
		`;

		// Add the rule to the document
		document.head.appendChild(style);

		if (!domEl.current) return;
		const dataUrl = await htmlToImage.toPng(domEl.current);

		document.head.removeChild(style);

		// download image
		const link = document.createElement('a');
		link.download = 'test.png';
		link.href = dataUrl;
		link.click();
	};

	useEffect(() => window.parent.postMessage('spacedrive-hello', '*'), []);

	if (import.meta.env.VITE_SD_DEMO_MODE === 'true') {
		hydrate(queryClient, demoData);
	}

	useEffect(() => {
		// if showControls then make K keybind take screenshot
		if (showControls) {
			window.addEventListener('keyup', (e) => {
				if (e.key === 'k') {
					downloadImage();
				}
			});
		}
	}, []);

	return (
		<div ref={domEl} className="App">
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
