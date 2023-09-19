import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { dialog, invoke, os, shell } from '@tauri-apps/api';
import { confirm } from '@tauri-apps/api/dialog';
import { listen } from '@tauri-apps/api/event';
import { homeDir } from '@tauri-apps/api/path';
import { onUpdaterEvent } from '@tauri-apps/api/updater';
import { appWindow } from '@tauri-apps/api/window';
import { useEffect } from 'react';
import { createBrowserRouter } from 'react-router-dom';
import { RspcProvider } from '@sd/client';
import {
	ErrorPage,
	KeybindEvent,
	OperatingSystem,
	Platform,
	PlatformProvider,
	routes,
	SpacedriveInterface
} from '@sd/interface';
import { getSpacedropState } from '@sd/interface/hooks/useSpacedropState';

import '@sd/ui/style';

import * as commands from './commands';

// TODO: Bring this back once upstream is fixed up.
// const client = hooks.createClient({
// 	links: [
// 		loggerLink({
// 			enabled: () => getDebugState().rspcLogger
// 		}),
// 		tauriLink()
// 	]
// });

async function getOs(): Promise<OperatingSystem> {
	switch (await os.type()) {
		case 'Linux':
			return 'linux';
		case 'Windows_NT':
			return 'windows';
		case 'Darwin':
			return 'macOS';
		default:
			return 'unknown';
	}
}

let customUriServerUrl = (window as any).__SD_CUSTOM_URI_SERVER__ as string | undefined;
const customUriAuthToken = (window as any).__SD_CUSTOM_SERVER_AUTH_TOKEN__ as string | undefined;
const startupError = (window as any).__SD_ERROR__ as string | undefined;

if (customUriServerUrl === undefined || customUriServerUrl === '')
	console.warn("'window.__SD_CUSTOM_URI_SERVER__' may have not been injected correctly!");
if (customUriServerUrl && !customUriServerUrl?.endsWith('/')) {
	customUriServerUrl += '/';
}
const queryParams = customUriAuthToken ? `?token=${encodeURIComponent(customUriAuthToken)}` : '';

const platform: Platform = {
	platform: 'tauri',
	getThumbnailUrlByThumbKey: (keyParts) =>
		`${customUriServerUrl}thumbnail/${keyParts
			.map((i) => encodeURIComponent(i))
			.join('/')}.webp${queryParams}`,
	getFileUrl: (libraryId, locationLocalId, filePathId) =>
		`${customUriServerUrl}file/${libraryId}/${locationLocalId}/${filePathId}${queryParams}`,
	openLink: shell.open,
	getOs,
	openDirectoryPickerDialog: () => dialog.open({ directory: true }),
	openFilePickerDialog: () => dialog.open(),
	saveFilePickerDialog: (opts) => dialog.save(opts),
	showDevtools: () => invoke('show_devtools'),
	confirm: (msg, cb) => confirm(msg).then(cb),
	userHomeDir: homeDir,
	updater: {
		checkForUpdate: async () => {
			const update = await commands.checkForUpdate();

			console.log(update);

			if (update === 'InProgress') return null;
			if ('Fetched' in update) return update.Fetched;
			return null;
		},
		installUpdate: commands.installUpdate
	},
	...commands
};

const queryClient = new QueryClient({
	defaultOptions: {
		queries: {
			networkMode: 'always'
		},
		mutations: {
			networkMode: 'always'
		}
	}
});

const router = createBrowserRouter(routes);

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
					<AppInner />
				</QueryClientProvider>
			</PlatformProvider>
		</RspcProvider>
	);
}

// This is required because `ErrorPage` uses the OS which comes from `PlatformProvider`
function AppInner() {
	if (startupError) {
		return (
			<ErrorPage
				message={startupError}
				submessage="Error occurred starting up the Spacedrive core"
			/>
		);
	}

	return <SpacedriveInterface router={router} />;
}
