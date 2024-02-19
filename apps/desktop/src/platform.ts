import { dialog, invoke, os, shell } from '@tauri-apps/api';
import { confirm } from '@tauri-apps/api/dialog';
import { homeDir } from '@tauri-apps/api/path';
import { open } from '@tauri-apps/api/shell';
// @ts-expect-error: Doesn't have a types package.
import ConsistentHash from 'consistent-hash';
import { OperatingSystem, Platform } from '@sd/interface';

import { commands, events } from './commands';
import { env } from './env';
import { createUpdater } from './updater';

const customUriAuthToken = (window as any).__SD_CUSTOM_SERVER_AUTH_TOKEN__ as string | undefined;
const customUriServerUrl = (window as any).__SD_CUSTOM_URI_SERVER__ as string[] | undefined;

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

let hr: typeof ConsistentHash | undefined;

function constructServerUrl(urlSuffix: string) {
	if (!hr) {
		if (!customUriServerUrl)
			throw new Error("'window.__SD_CUSTOM_URI_SERVER__' was not injected correctly!");

		hr = new ConsistentHash();
		customUriServerUrl.forEach((url) => hr.add(url));
	}

	// Randomly switch between servers to avoid HTTP connection limits
	return hr.get(urlSuffix) + urlSuffix + queryParams;
}

export const platform = {
	platform: 'tauri',
	getThumbnailUrlByThumbKey: (keyParts) =>
		constructServerUrl(
			`/thumbnail/${keyParts.map((i) => encodeURIComponent(i)).join('/')}.webp`
		),
	getFileUrl: (libraryId, locationLocalId, filePathId) =>
		constructServerUrl(`/file/${libraryId}/${locationLocalId}/${filePathId}`),
	getFileUrlByPath: (path) =>
		constructServerUrl(`/local-file-by-path/${encodeURIComponent(path)}`),
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
	subscribeToDragAndDropEvents: (cb) =>
		events.dragAndDropEvent.listen((e) => {
			cb(e.payload);
		}),
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
