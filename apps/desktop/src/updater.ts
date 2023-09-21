import { listen } from '@tauri-apps/api/event';
import { useEffect, useRef } from 'react';
import { proxy, useSnapshot } from 'valtio';
import { UpdateStore } from '@sd/interface';
import { toast, ToastId } from '@sd/ui';

import * as commands from './commands';

export const updateStore = proxy<UpdateStore>({
	status: 'idle'
});

listen<UpdateStore>('updater', (e) => {
	Object.assign(updateStore, e.payload);
	console.log(updateStore);
});

const onInstallCallbacks = new Set<() => void>();

export const updater = {
	useSnapshot: () => useSnapshot(updateStore),
	checkForUpdate: commands.checkForUpdate,
	installUpdate: () => {
		for (const cb of onInstallCallbacks) {
			cb();
		}

		const promise = commands.installUpdate();

		toast.promise(promise, {
			loading: 'Downloading Update',
			success: 'Update Downloaded. Restart Spacedrive to install',
			error: 'Failed to download update'
		});

		return promise;
	}
};

async function checkForUpdate() {
	const update = await updater.checkForUpdate();

	if (!update) return;

	let id: ToastId | null = null;

	const cb = () => {
		if (id !== null) toast.dismiss(id);
	};

	onInstallCallbacks.add(cb);

	toast.info(
		(_id) => {
			id = _id;

			return {
				title: 'New Update Available',
				body: `Version ${update.version}`
			};
		},
		{
			onClose() {
				onInstallCallbacks.delete(cb);
			},
			duration: 10 * 1000,
			action: {
				label: 'Update',
				onClick: () => updater.installUpdate()
			}
		}
	);
}

export function useUpdater() {
	const alreadyChecked = useRef(false);

	useEffect(() => {
		if (!alreadyChecked.current && import.meta.env.PROD) checkForUpdate();
		alreadyChecked.current = true;
	}, []);
}
