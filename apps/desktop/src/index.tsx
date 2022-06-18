// import Spacedrive JS client
import { BaseTransport } from '@sd/client';
// import types from Spacedrive core (TODO: re-export from client would be cleaner)
import { ClientCommand, ClientQuery, CoreEvent } from '@sd/core';
// import Spacedrive interface
import SpacedriveInterface, { Platform } from '@sd/interface';
// import tauri apis
import { dialog, invoke, os, shell } from '@tauri-apps/api';
import { Event, listen } from '@tauri-apps/api/event';
import { convertFileSrc } from '@tauri-apps/api/tauri';
import { appWindow } from '@tauri-apps/api/window';
import React, { useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';

import '@sd/ui/style';

// bind state to core via Tauri
class Transport extends BaseTransport {
	constructor() {
		super();

		listen('core_event', (e: Event<CoreEvent>) => {
			this.emit('core_event', e.payload);
		});
	}
	async query(query: ClientQuery) {
		return await invoke('client_query_transport', { data: query });
	}
	async command(query: ClientCommand) {
		return await invoke('client_command_transport', { data: query });
	}
}

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
		os.platform().then((platform) => setPlatform(getPlatform(platform)));
		invoke('app_ready');
	}, []);

	useEffect(() => {
		const unlistenFocus = listen('tauri://focus', () => setFocused(true));
		const unlistenBlur = listen('tauri://blur', () => setFocused(false));

		return () => {
			unlistenFocus.then((unlisten) => unlisten());
			unlistenBlur.then((unlisten) => unlisten());
		};
	}, []);

	return (
		<SpacedriveInterface
			transport={new Transport()}
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
	);
}

const root = createRoot(document.getElementById('root')!);

root.render(
	<React.StrictMode>
		<App />
	</React.StrictMode>
);
