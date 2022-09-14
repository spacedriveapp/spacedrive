// import Spacedrive JS client
import { createClient } from '@rspc/client';
import { TauriTransport } from '@rspc/tauri';
import { Operations, queryClient, rspc } from '@sd/client';
import SpacedriveInterface, { Platform } from '@sd/interface';
import { KeybindEvent } from '@sd/interface';
import { dialog, invoke, os, shell } from '@tauri-apps/api';
import { listen } from '@tauri-apps/api/event';
import { convertFileSrc } from '@tauri-apps/api/tauri';
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

	const [platform, setPlatform] = useState<Platform>('unknown');
	const [focused, setFocused] = useState(true);

	useEffect(() => {
		os.platform().then((platform) => setPlatform(getPlatform(platform)));
		invoke('app_ready');
	}, []);

	useEffect(() => {
		const focusListener = listen('tauri://focus', () => setFocused(true));
		const blurListener = listen('tauri://blur', () => setFocused(false));
		const keybindListener = listen('keybind', (input) => {
			document.dispatchEvent(new KeybindEvent(input.payload as string));
		});

		return () => {
			focusListener.then((unlisten) => unlisten());
			blurListener.then((unlisten) => unlisten());
			keybindListener.then((unlisten) => unlisten());
		};
	}, []);

	return (
		<rspc.Provider client={client} queryClient={queryClient}>
			<SpacedriveInterface
				platform={platform}
				convertFileSrc={function (url: string): string {
					return convertFileSrc(url);
				}}
				openDialog={function (options: {
					directory?: boolean | undefined;
				}): Promise<string | string[] | null> {
					return dialog.open(options);
				}}
				isFocused={focused}
				onClose={() => appWindow.close()}
				onFullscreen={() => appWindow.setFullscreen(true)}
				onMinimize={() => appWindow.minimize()}
				onOpen={(path: string) => shell.open(path)}
			/>
		</rspc.Provider>
	);
}

const root = createRoot(document.getElementById('root')!);

root.render(
	<React.StrictMode>
		<App />
	</React.StrictMode>
);
