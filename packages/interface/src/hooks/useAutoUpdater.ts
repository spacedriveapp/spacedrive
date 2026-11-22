import {toast} from '@spacedrive/primitives';
import {useEffect} from 'react';
import {usePlatform, type AvailableUpdate} from '../contexts/PlatformContext';

/** How often a running app checks the release feed after the startup check. */
export const UPDATE_CHECK_INTERVAL_MS = 6 * 60 * 60 * 1000;

/**
 * Checks for updates now and then on every interval, calling onUpdate once
 * per new version so a dismissed prompt does not return every interval.
 *
 * A failed check (offline, feed missing, no build for this platform) only
 * logs at debug level: the next interval tries again, and nothing the user
 * can act on went wrong. Returns a function that stops the checks.
 */
export function watchForUpdates(options: {
	check: () => Promise<AvailableUpdate | null>;
	onUpdate: (update: AvailableUpdate) => void;
	intervalMs?: number;
}): () => void {
	const {check, onUpdate, intervalMs = UPDATE_CHECK_INTERVAL_MS} = options;
	let stopped = false;
	let promptedVersion: string | undefined;

	const run = async () => {
		try {
			const update = await check();
			if (stopped || !update || update.version === promptedVersion)
				return;
			promptedVersion = update.version;
			onUpdate(update);
		} catch (error) {
			console.debug('Update check failed', error);
		}
	};

	void run();
	const timer = setInterval(run, intervalMs);
	return () => {
		stopped = true;
		clearInterval(timer);
	};
}

async function install(update: AvailableUpdate) {
	const id = toast.info(`Downloading Spacedrive ${update.version}...`, {
		duration: Infinity
	});
	try {
		await update.installAndRestart((percent) => {
			if (percent === undefined) return;
			toast.info(
				`Downloading Spacedrive ${update.version}... ${percent}%`,
				{
					id,
					duration: Infinity
				}
			);
		});
	} catch (error) {
		toast.error(
			{
				title: 'Update failed',
				body: error instanceof Error ? error.message : String(error)
			},
			{id}
		);
	}
}

/**
 * Offers new desktop releases with a toast that stays until the user acts.
 *
 * Nothing downloads until the user picks "Install and restart". The platform
 * only provides checkForUpdate in release desktop builds, so dev builds and
 * the web app never check.
 */
export function useAutoUpdater() {
	const platform = usePlatform();
	const checkForUpdate = platform.checkForUpdate;

	useEffect(() => {
		if (!checkForUpdate) return;
		return watchForUpdates({
			check: checkForUpdate,
			onUpdate: (update) => {
				toast.info(
					{
						title: `Spacedrive ${update.version} is available`,
						body: `You have ${update.currentVersion}. Spacedrive restarts after the update installs.`
					},
					{
						duration: Infinity,
						action: {
							label: 'Install and restart',
							onClick: () => void install(update)
						},
						cancel: 'Later'
					}
				);
			}
		});
	}, [checkForUpdate]);
}
