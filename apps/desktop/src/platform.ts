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

export const platform = {
	platform: 'tauri',
	getThumbnailUrlByThumbKey: (keyParts) =>
		`${customUriServerUrl}thumbnail/${keyParts
			.map((i) => encodeURIComponent(i))
			.join('/')}.webp${queryParams}`,
	getFileUrl: (libraryId, locationLocalId, filePathId) =>
		`${customUriServerUrl}file/${libraryId}/${locationLocalId}/${filePathId}${queryParams}`,
	getFileUrlByPath: (path) =>
		`${customUriServerUrl}local-file-by-path/${encodeURIComponent(path)}${queryParams}`,
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
