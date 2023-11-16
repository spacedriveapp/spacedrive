import { createMemoryHistory } from '@remix-run/router';
import { QueryClientProvider } from '@tanstack/react-query';
import { listen } from '@tauri-apps/api/event';
import { appWindow } from '@tauri-apps/api/window';
import { useEffect, useState } from 'react';
import { RspcProvider } from '@sd/client';
import {
	ErrorPage,
	KeybindEvent,
	PlatformProvider,
	routes,
	SpacedriveInterface,
	TabsContext
} from '@sd/interface';
import { getSpacedropState } from '@sd/interface/hooks/useSpacedropState';

import '@sd/ui/style/style.scss';

import * as commands from './commands';
import { platform } from './platform';
import { queryClient } from './query';
import { createMemoryRouterWithHistory } from './router';

// TODO: Bring this back once upstream is fixed up.
// const client = hooks.createClient({
// 	links: [
// 		loggerLink({
// 			enabled: () => getDebugState().rspcLogger
// 		}),
// 		tauriLink()
// 	]
// });

const startupError = (window as any).__SD_ERROR__ as string | undefined;

export default function App() {
	useEffect(() => {
		// This tells Tauri to show the current window because it's finished loading
		commands.appReady();
	}, []);

	useEffect(() => {
		const keybindListener = listen('keybind', (input) => {
			document.dispatchEvent(new KeybindEvent(input.payload as string));
		});

		const dropEventListener = appWindow.onFileDropEvent((event) => {
			if (event.payload.type === 'drop') {
				getSpacedropState().droppedFiles = event.payload.paths;
			}
		});

		return () => {
			keybindListener.then((unlisten) => unlisten());
			dropEventListener.then((unlisten) => unlisten());
		};
	}, []);

	return (
		<RspcProvider queryClient={queryClient}>
			<PlatformProvider platform={platform}>
				<QueryClientProvider client={queryClient}>
					{startupError ? (
						<ErrorPage
							message={startupError}
							submessage="Error occurred starting up the Spacedrive core"
						/>
					) : (
						<AppInner />
					)}
				</QueryClientProvider>
			</PlatformProvider>
		</RspcProvider>
	);
}

function AppInner() {
	function createRouter() {
		const history = createMemoryHistory();
		const router = createMemoryRouterWithHistory({ routes, history });

		const dispose = router.subscribe((event) => {
			setRouters((routers) => {
				const index = routers.findIndex((r) => r.router === router);
				if (index === -1) return routers;

				const routerAtIndex = routers[index]!;

				routers[index] = {
					...routerAtIndex,
					currentIndex: history.index,
					maxIndex:
						event.historyAction === 'PUSH'
							? history.index
							: Math.max(routerAtIndex.maxIndex, history.index)
				};

				return [...routers];
			});
		});

		return {
			router,
			history,
			dispose,
			currentIndex: 0,
			maxIndex: 0
		};
	}

	const [routers, setRouters] = useState(() => [createRouter()]);
	const [routerIndex, setRouterIndex] = useState(0);

	const router = routers[routerIndex]!;

	return (
		<TabsContext.Provider
			value={{
				routerIndex,
				setRouterIndex,
				routers: routers.map(({ router }) => router),
				createRouter() {
					setRouters((r) => [...r, createRouter()]);
				},
				removeRouter(index: number) {
					setRouters((routers) => {
						const router = routers[index];

						if (!router) return routers;

						router.dispose();

						routers.splice(index, 1);

						setRouterIndex(routers.length - 1);

						return [...routers];
					});
				}
			}}
		>
			<SpacedriveInterface
				routing={{
					router: router.router,
					routers: routers.map((r) => r.router),
					currentIndex: router.currentIndex,
					maxIndex: router.maxIndex
				}}
			/>
		</TabsContext.Provider>
	);
}
