import { useEffect } from 'react';
import { createMutable } from 'solid-js/store';

import type { BackendFeature } from '../core';
import { useBridgeQuery } from '../rspc';
import { createPersistedMutable, useObserver } from '../solid';

export const features = [
	'backups',
	'debugRoutes',
	'solidJsDemo',
	'hostedLocations',
	'debugDragAndDrop',
	'searchTargetSwitcher',
	'wipP2P'
] as const;

// This defines which backend feature flags show up in the UI.
// This is kinda a hack to not having the runtime array of possible features as Specta only exports the types.
export const backendFeatures: BackendFeature[] = [];

export type FeatureFlag = (typeof features)[number] | BackendFeature;

export const featureFlagsStore = createPersistedMutable(
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
		featureFlagsStore.enabled = [
			// Remove all backend features.
			...featureFlagsStore.enabled.filter((f) => features.includes(f as any)),
			// Add back in current state of backend features

			...(nodeConfig.data?.features ?? [])
		];
	}, [nodeConfig.data?.features]);
}

export function useFeatureFlags() {
	// We have to be special here.
	// `useSolidStore` would not work as it "subscribes" to the array, not the items in the array.
	return useObserver(() => [...featureFlagsStore.enabled]);
}

export function useFeatureFlag(flag: FeatureFlag | FeatureFlag[]) {
	useFeatureFlags(); // Rerender on change
	return Array.isArray(flag) ? flag.every((f) => isEnabled(f)) : isEnabled(flag);
}

export const isEnabled = (flag: FeatureFlag) =>
	featureFlagsStore.enabled.find((ff) => flag === ff) !== undefined;

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
				const result = featureFlagsStore.enabled.find((ff) => f === ff)
					? true
					: await confirm(
							'This feature will render your database broken and it WILL need to be reset! Use at your own risk!'
						);

				if (result) {
					// nonLibraryClient.mutation(['toggleFeatureFlag', f as any]);
				}
			})();

			return;
		}

		if (!featureFlagsStore.enabled.find((ff) => f === ff)) {
			let message: string | undefined;
			if (f === 'backups') {
				message =
					'Backups are done on your live DB without proper Sqlite snapshotting. This will work but it could result in unintended side so be careful!';
			}

			if (message) {
				void (async () => {
					// Tauri's `confirm` returns a promise but it's not typesafe
					const result = await confirm(message);

					if (result) {
						featureFlagsStore.enabled.push(f);
					}
				})();
			} else {
				featureFlagsStore.enabled.push(f);
			}
		} else {
			featureFlagsStore.enabled = featureFlagsStore.enabled.filter((ff) => f !== ff);
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
