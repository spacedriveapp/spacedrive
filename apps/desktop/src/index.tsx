import { loggerLink, splitLink } from '@rspc/client';
import { tauriLink } from '@rspc/tauri';
import { OperatingSystem, PlatformProvider, getDebugState, hooks, queryClient } from '@sd/client';
import SpacedriveInterface, { Platform } from '@sd/interface';
import { KeybindEvent } from '@sd/interface';
import { dialog, invoke, os, shell } from '@tauri-apps/api';
import { listen } from '@tauri-apps/api/event';
import React, { useEffect } from 'react';
import { createRoot } from 'react-dom/client';

import '@sd/ui/style';

const isDev = import.meta.env.DEV;
const client = hooks.createClient({
	links: [
		splitLink({
			condition: () => getDebugState().rspcLogger,
			true: [loggerLink(), tauriLink()],
			false: [tauriLink()]
		})
	]
});

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

const platform: Platform = {
	platform: 'tauri',
	getThumbnailUrlById: (casId) => `spacedrive://thumbnail/${encodeURIComponent(casId)}`,
	openLink: shell.open,
	getOs,
	openFilePickerDialog: () => dialog.open({ directory: true }),
	showDevtools: () => invoke('show_devtools'),
	openPath: (path) => shell.open(path)
};

function App() {
	useEffect(() => {
		// This tells Tauri to show the current window because it's finished loading
		invoke('app_ready');
	}, []);

	useEffect(() => {
		const keybindListener = listen('keybind', (input) => {
			document.dispatchEvent(new KeybindEvent(input.payload as string));
		});

		return () => {
			keybindListener.then((unlisten) => unlisten());
		};
	}, []);

	return (
		<hooks.Provider client={client} queryClient={queryClient}>
			<PlatformProvider platform={platform}>
				<SpacedriveInterface />
			</PlatformProvider>
		</hooks.Provider>
	);
}

const root = createRoot(document.getElementById('root')!);

root.render(
	<React.StrictMode>
		<App />
	</React.StrictMode>
);
