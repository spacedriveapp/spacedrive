import { createClient } from '@rspc/client';
import { TauriTransport } from '@rspc/tauri';
import { OperatingSystem, Operations, PlatformProvider, queryClient, rspc } from '@sd/client';
import SpacedriveInterface, { Platform } from '@sd/interface';
import { KeybindEvent } from '@sd/interface';
import { dialog, invoke, os } from '@tauri-apps/api';
import { listen } from '@tauri-apps/api/event';
import React, { useEffect } from 'react';
import { createRoot } from 'react-dom/client';

import '@sd/ui/style';

const client = createClient<Operations>({
	transport: new TauriTransport()
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
	openLink: open,
	getOs,
	openFilePickerDialog: () => dialog.open({ directory: true })
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
		<rspc.Provider client={client} queryClient={queryClient}>
			<PlatformProvider platform={platform}>
				<SpacedriveInterface />
			</PlatformProvider>
		</rspc.Provider>
	);
}

const root = createRoot(document.getElementById('root')!);

root.render(
	<React.StrictMode>
		<App />
	</React.StrictMode>
);
