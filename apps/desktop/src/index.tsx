// import Spacedrive JS client
import { TauriTransport, createClient } from '@rspc/client';
import { Operations, queryClient, rspc } from '@sd/client';
import SpacedriveInterface, { Platform } from '@sd/interface';
import { dialog, invoke, os, shell } from '@tauri-apps/api';
import { listen } from '@tauri-apps/api/event';
import { appWindow } from '@tauri-apps/api/window';
import React, { useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';

import '@sd/ui/style';

const client = createClient<Operations>({
	transport: new TauriTransport()
});

function App() {
	function getPlatform(platform: string): Platform {
		switch (platform) {
			case 'darwin':
				return 'macOS';
			case 'win32':
				return 'windows';
			case 'linux':
				return 'linux';
			default:
				return 'browser';
		}
	}

	const [platform, setPlatform] = useState<Platform>('macOS');
	const [focused, setFocused] = useState(true);

	useEffect(() => {
		os.platform()
			.then((platform) => setPlatform(getPlatform(platform)))
			.catch(console.error);
		invoke('app_ready').catch(console.error);
	}, []);

	useEffect(() => {
		const unlistenFocus = listen('tauri://focus', () => setFocused(true));
		const unlistenBlur = listen('tauri://blur', () => setFocused(false));

		return () => {
			unlistenFocus.then((unlisten) => unlisten()).catch(console.error);
			unlistenBlur.then((unlisten) => unlisten()).catch(console.error);
		};
	}, []);

	return (
		<rspc.Provider client={client} queryClient={queryClient}>
			<SpacedriveInterface
				platform={platform}
				getThumbnailUrlById={(casId: string) =>
					`spacedrive://thumbnail/${encodeURIComponent(casId)}`
				}
				openDialog={async (options: {
					directory?: boolean | undefined;
				}): Promise<string | string[] | null> => {
					return await dialog.open(options);
				}}
				isFocused={focused}
				onClose={() => {
					appWindow.close().catch((err) => console.error('Unable to close window!', err));
				}}
				onFullscreen={() => {
					appWindow
						.setFullscreen(true)
						.catch((err) => console.error('Unable to fullscreen window!', err));
				}}
				onMinimize={() => {
					appWindow.minimize().catch((err) => console.error('Unable to minimize window!', err));
				}}
				onOpen={(path: string) => {
					shell.open(path).catch((err) => console.error('Unable to open file!', err));
				}}
			/>
		</rspc.Provider>
	);
}

// eslint-disable-next-line @typescript-eslint/no-non-null-assertion
const root = createRoot(document.getElementById('root')!);

root.render(
	<React.StrictMode>
		<App />
	</React.StrictMode>
);
