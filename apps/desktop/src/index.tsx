import { TauriTransport, createClient } from '@rspc/client';
import { OperatingSystem, Operations, PlatformProvider, queryClient, rspc } from '@sd/client';
import SpacedriveInterface, { Platform } from '@sd/interface';
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
	getOs,
	openFilePickerDialog: () => dialog.open({ directory: true })
};

function App() {
	useEffect(() => {
		// This tells Tauri to show the current window because it's finished loading
		invoke('app_ready');

		// This is a hacky solution to run the action items in the macOS menu bar by executing their keyboard shortcuts in the DOM.
		// This means we can build shortcuts that work on web while calling them like native actions.
		const unlisten = listen('do_keyboard_input', (input) => {
			document.dispatchEvent(new KeyboardEvent('keydown', input.payload as any));
		});

		return () => {
			unlisten.then((unlisten) => unlisten());
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
