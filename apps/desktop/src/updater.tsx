import { listen } from '@tauri-apps/api/event';
import { proxy, useSnapshot } from 'valtio';
import { UpdateStore } from '@sd/interface';
import { toast, ToastId } from '@sd/ui';

import * as commands from './commands';

declare global {
	interface Window {
		__SD_UPDATER__?: true;
	}
}

export function createUpdater() {
	if (!window.__SD_UPDATER__) return;

	const updateStore = proxy<UpdateStore>({
		status: 'idle'
	});

	listen<UpdateStore>('updater', (e) => {
		Object.assign(updateStore, e.payload);
		console.log(updateStore);
	});

	const onInstallCallbacks = new Set<() => void>();

	async function checkForUpdate() {
		const update = await commands.checkForUpdate();

		if (!update) return null;

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
					onClick: installUpdate
				}
			}
		);

		return update;
	}

	function installUpdate() {
		for (const cb of onInstallCallbacks) {
			cb();
		}

		const promise = commands.installUpdate();

		toast.promise(promise, {
			loading: 'Downloading Update',
			success: 'Update Downloaded. Restart Spacedrive to install',
			error: (e: any) => (
				<>
					<p>Failed to download update</p>
					<p className="text-gray-300">Error: {e.toString()}</p>
				</>
			)
		});

		return promise;
	}

	return {
		useSnapshot: () => useSnapshot(updateStore),
		checkForUpdate,
		installUpdate
	};
}
