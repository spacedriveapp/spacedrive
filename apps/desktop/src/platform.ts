import { dialog, invoke, os, shell } from '@tauri-apps/api';
import { confirm } from '@tauri-apps/api/dialog';
import { homeDir } from '@tauri-apps/api/path';
import { open } from '@tauri-apps/api/shell';
import { OperatingSystem, Platform } from '@sd/interface';

import { commands } from './commands';
import { env } from './env';
import { createUpdater } from './updater';

const customUriAuthToken = (window as any).__SD_CUSTOM_SERVER_AUTH_TOKEN__ as string | undefined;
let customUriServerUrl = (window as any).__SD_CUSTOM_URI_SERVER__ as string | undefined;
const startPort = (window as any).__SD_START_PORT__ as number | undefined;

if (customUriServerUrl === undefined || customUriServerUrl === '')
	console.warn("'window.__SD_CUSTOM_URI_SERVER__' may have not been injected correctly!");
if (customUriServerUrl && !customUriServerUrl?.endsWith('/')) {
	customUriServerUrl += '/';
}

const queryParams = customUriAuthToken ? `?token=${encodeURIComponent(customUriAuthToken)}` : '';

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

function randomIntFromInterval(min: number, max: number) {
	// min and max included
	return Math.floor(Math.random() * (max - min + 1) + min);
}

function randomPort() {
	if (!startPort) throw new Error(`'startPort' not defined`);
	return randomIntFromInterval(startPort, startPort + 3); // Randomly switch between 4 servers
}

export const platform = {
	platform: 'tauri',
	getThumbnailUrlByThumbKey: (keyParts) => {
		const url = `http://localhost:${randomPort()}/`;
		// console.log('A', customUriServerUrl, url);
		return `${url}thumbnail/${keyParts
			.map((i) => encodeURIComponent(i))
			.join('/')}.webp${queryParams}`;
	},
	getFileUrl: (libraryId, locationLocalId, filePathId) => {
		const url = `http://localhost:${randomPort()}/`;
		// console.log('B', customUriServerUrl, url);
		return `${url}file/${libraryId}/${locationLocalId}/${filePathId}${queryParams}`;
	},
	getFileUrlByPath: (path) => {
		const url = `http://localhost:${randomPort()}/`;
		// console.log('C', customUriServerUrl, url);
		return `${url}local-file-by-path/${encodeURIComponent(path)}${queryParams}`;
	},
	openLink: shell.open,
	getOs,
	openDirectoryPickerDialog: (opts) => {
		const result = dialog.open({ directory: true, ...opts });
		if (opts?.multiple) return result as any; // Tauri don't properly type narrow on `multiple` argument
		return result;
	},
	openFilePickerDialog: () => dialog.open(),
	saveFilePickerDialog: (opts) => dialog.save(opts),
	showDevtools: () => invoke('show_devtools'),
	confirm: (msg, cb) => confirm(msg).then(cb),
	userHomeDir: homeDir,
	updater: window.__SD_UPDATER__ ? createUpdater() : undefined,
	auth: {
		start(url) {
			open(url);
		}
	},
	...commands,
	landingApiOrigin: env.VITE_LANDING_ORIGIN
} satisfies Platform;
