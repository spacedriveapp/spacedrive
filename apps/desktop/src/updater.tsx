import { listen } from '@tauri-apps/api/event';
import { proxy, useSnapshot } from 'valtio';
import { UpdateStore } from '@sd/interface';
import { useLocale } from '@sd/interface/hooks';
import { toast, ToastId } from '@sd/ui';

import { commands } from './commands';

declare global {
	interface Window {
		__SD_UPDATER__?: true;
		__SD_DESKTOP_VERSION__: string;
	}
}

export function createUpdater(t: ReturnType<typeof useLocale>['t']) {
	if (!window.__SD_UPDATER__) return;

	const updateStore = proxy<UpdateStore>({
		status: 'idle'
	});

	listen<UpdateStore>('updater', (e) => {
		Object.assign(updateStore, e.payload);
	});

	const onInstallCallbacks = new Set<() => void>();

	async function checkForUpdate() {
		const result = await commands.checkForUpdate();

		if (result.status === 'error') {
			console.error('UPDATER ERROR', result.error);
			// TODO: Show some UI?
			return null;
		}
		if (!result.data) return null;
		const update = result.data;

		let id: ToastId | null = null;

		const cb = () => {
			if (id !== null) toast.dismiss(id);
		};

		onInstallCallbacks.add(cb);

		toast.info(
			(_id) => {
				const { t } = useLocale();

				id = _id;

				return {
					title: t('new_update_available'),
					body: t('version', { version: update.version })
				};
			},
			{
				onClose() {
					onInstallCallbacks.delete(cb);
				},
				duration: 10 * 1000,
				action: {
					label: t('update'),
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
			loading: t('downloading_update'),
			success: t('update_downloaded'),
			error: (e: any) => (
				<>
					<p>{t('failed_to_download_update')}</p>
					<p className="text-gray-300">Error: {e.toString()}</p>
				</>
			)
		});

		return promise;
	}

	const SD_VERSION_LOCALSTORAGE = 'sd-version';
	async function runJustUpdatedCheck(onViewChangelog: () => void) {
		const version = window.__SD_DESKTOP_VERSION__;
		const lastVersion = localStorage.getItem(SD_VERSION_LOCALSTORAGE);
		if (!lastVersion) return;

		if (lastVersion !== version) {
			localStorage.setItem(SD_VERSION_LOCALSTORAGE, version);
			let tagline = null;

			try {
				const request = await fetch(
					`${import.meta.env.VITE_LANDING_ORIGIN}/api/releases/${version}`
				);
				const { frontmatter } = await request.json();
				tagline = frontmatter?.tagline;
			} catch (error) {
				console.warn('Failed to fetch release info');
				console.error(error);
			}

			toast.success(
				{
					title: t('updated_successfully', { version }),
					body: tagline
				},
				{
					duration: 10 * 1000,
					action: {
						label: t('view_changes'),
						onClick: onViewChangelog
					}
				}
			);
		}
	}

	return {
		useSnapshot: () => useSnapshot(updateStore),
		checkForUpdate,
		installUpdate,
		runJustUpdatedCheck
	};
}
