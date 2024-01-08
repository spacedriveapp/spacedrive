import { useEffect } from 'react';
import { createMutable } from 'solid-js/store';

import type { BackendFeature } from '../core';
import { nonLibraryClient, useBridgeQuery } from '../rspc';
import { createPersistedMutable, useSolidStore } from '../solidjs-interop';

export const features = [
	'spacedrop',
	'p2pPairing',
	'backups',
	'debugRoutes',
	'solidJsDemo',
	'hostedLocations'
] as const;

// This defines which backend feature flags show up in the UI.
// This is kinda a hack to not having the runtime array of possible features as Specta only exports the types.
export const backendFeatures: BackendFeature[] = ['syncEmitMessages', 'filesOverP2P', 'cloudSync'];

export type FeatureFlag = (typeof features)[number] | BackendFeature;

export const featureFlagState = createPersistedMutable(
	'sd-featureFlags',
	createMutable({
		enabled: [] as FeatureFlag[]
	}),
	{
		onSave: (data) => {
			// Clone so we don't mess with the original data
			const data2: typeof data = JSON.parse(JSON.stringify(data));
			// Only save frontend flags (backend flags are saved in the backend)
			data2.enabled = data2.enabled.filter((f) => features.includes(f as any));
			return data2;
		}
	}
);

export function useLoadBackendFeatureFlags() {
	const nodeConfig = useBridgeQuery(['nodeState']);

	useEffect(() => {
		featureFlagState.enabled = [
			// Remove all backend features.
			...featureFlagState.enabled.filter((f) => features.includes(f as any)),
			// Add back in current state of backend features

			...(nodeConfig.data?.features ?? [])
		];
	}, [nodeConfig.data?.features]);
}

export function useFeatureFlags() {
	return useSolidStore(featureFlagState);
}

export function useFeatureFlag(flag: FeatureFlag | FeatureFlag[]) {
	useSolidStore(featureFlagState); // Rerender on change
	return Array.isArray(flag) ? flag.every((f) => isEnabled(f)) : isEnabled(flag);
}

export const isEnabled = (flag: FeatureFlag) =>
	featureFlagState.enabled.find((ff) => flag === ff) !== undefined;

export function toggleFeatureFlag(flags: FeatureFlag | FeatureFlag[]) {
	if (!Array.isArray(flags)) {
		flags = [flags];
	}
	flags.forEach((f) => {
		// If not in `features` it must be a backend feature
		if (!features.includes(f as any)) {
			void (async () => {
				// Tauri's `confirm` returns a Promise
				// Only prompt when enabling the feature
				const result = featureFlagState.enabled.find((ff) => f === ff)
					? true
					: await confirm(
							'This feature will render your database broken and it WILL need to be reset! Use at your own risk!'
					  );

				if (result) {
					nonLibraryClient.mutation(['toggleFeatureFlag', f as any]);
				}
			})();

			return;
		}

		if (!featureFlagState.enabled.find((ff) => f === ff)) {
			let message: string | undefined;
			if (f === 'p2pPairing') {
				message =
					'This feature will render your database broken and it WILL need to be reset! Use at your own risk!';
			} else if (f === 'backups') {
				message =
					'Backups are done on your live DB without proper Sqlite snapshotting. This will work but it could result in unintended side so be careful!';
			}

			if (message) {
				void (async () => {
					// Tauri's `confirm` returns a promise but it's not typesafe
					const result = await confirm(message);

					if (result) {
						featureFlagState.enabled.push(f);
					}
				})();
			} else {
				featureFlagState.enabled.push(f);
			}
		} else {
			featureFlagState.enabled = featureFlagState.enabled.filter((ff) => f !== ff);
		}
	});
}

// Render component only when feature flag is enabled
export function withFeatureFlag(
	flag: FeatureFlag | FeatureFlag[],
	Component: React.FunctionComponent,
	fallback: React.ReactNode = null
): React.FunctionComponent {
	return (props) => {
		const enabled = useFeatureFlag(flag);
		return enabled ? <Component /> : fallback;
	};
}
