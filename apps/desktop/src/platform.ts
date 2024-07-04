import { invoke } from '@tauri-apps/api/core';
import { homeDir } from '@tauri-apps/api/path';
import { confirm, open as dialogOpen, save as dialogSave } from '@tauri-apps/plugin-dialog';
import { type } from '@tauri-apps/plugin-os';
import { open as shellOpen } from '@tauri-apps/plugin-shell';
// @ts-expect-error: Doesn't have a types package.
import ConsistentHash from 'consistent-hash';
import { OperatingSystem, Platform } from '@sd/interface';

import { commands, events } from './commands';
import { env } from './env';

const customUriAuthToken = (window as any).__SD_CUSTOM_SERVER_AUTH_TOKEN__ as string | undefined;
const customUriServerUrl = (window as any).__SD_CUSTOM_URI_SERVER__ as string[] | undefined;

const queryParams = customUriAuthToken ? `?token=${encodeURIComponent(customUriAuthToken)}` : '';

async function getOs(): Promise<OperatingSystem> {
	switch (await type()) {
		case 'linux':
			return 'linux';
		case 'windows':
			return 'windows';
		case 'macos':
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
	getThumbnailUrlByThumbKey: (thumbKey) =>
		constructServerUrl(
			`/thumbnail/${encodeURIComponent(
				thumbKey.base_directory_str
			)}/${encodeURIComponent(thumbKey.shard_hex)}/${encodeURIComponent(thumbKey.cas_id)}.webp`
		),
	getFileUrl: (libraryId, locationLocalId, filePathId) =>
		constructServerUrl(`/file/${libraryId}/${locationLocalId}/${filePathId}`),
	getFileUrlByPath: (path) =>
		constructServerUrl(`/local-file-by-path/${encodeURIComponent(path)}`),
	getRemoteRspcEndpoint: (remote_identity) => ({
		url: `${customUriServerUrl?.[0]
			?.replace('https', 'wss')
			?.replace('http', 'ws')}/remote/${encodeURIComponent(
			remote_identity
		)}/rspc/ws?token=${customUriAuthToken}`
	}),
	constructRemoteRspcPath: (remote_identity, path) =>
		constructServerUrl(
			`/remote/${encodeURIComponent(remote_identity)}/uri/${path}?token=${customUriAuthToken}`
		),
	openLink: shellOpen,
	getOs,
	openDirectoryPickerDialog: (opts) => {
		const result = dialogOpen({ directory: true, ...opts });
		if (opts?.multiple) return result as any; // Tauri don't properly type narrow on `multiple` argument
		return result;
	},
	openFilePickerDialog: () =>
		dialogOpen({
			multiple: true
		}).then((result) => result?.map((r) => r.path) ?? null),
	saveFilePickerDialog: (opts) => dialogSave(opts),
	showDevtools: () => invoke('show_devtools'),
	confirm: (msg, cb) => confirm(msg).then(cb),
	subscribeToDragAndDropEvents: (cb) =>
		events.dragAndDropEvent.listen((e) => {
			cb(e.payload);
		}),
	userHomeDir: homeDir,
	auth: {
		start(url) {
			return shellOpen(url);
		}
	},
	...commands,
	landingApiOrigin: env.VITE_LANDING_ORIGIN
} satisfies Omit<Platform, 'updater'>;
