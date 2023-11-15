import { QueryClientProvider } from '@tanstack/react-query';
import { listen } from '@tauri-apps/api/event';
import { appWindow } from '@tauri-apps/api/window';
import { useEffect, useState } from 'react';
import { createMemoryRouter } from 'react-router-dom';
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

function createRouter() {
	return createMemoryRouter(routes);
}

function AppInner() {
	const [routers, setRouters] = useState(() => [createRouter(), createRouter()]);

	const [routerIndex, setRouterIndex] = useState(0);

	const router = routers[routerIndex]!;

	return (
		<TabsContext.Provider
			value={{
				routerIndex,
				setRouterIndex,
				routers,
				setRouters,
				createRouter
			}}
		>
			<SpacedriveInterface router={router} routers={routers} />
		</TabsContext.Provider>
	);
}
