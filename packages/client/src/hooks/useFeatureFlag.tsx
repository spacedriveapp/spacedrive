import { useEffect } from 'react';
import { subscribe, useSnapshot } from 'valtio';
import { valtioPersist } from '../lib/valito';

export const features = ['spacedrop', 'p2pPairing', 'syncRoute', 'backups'] as const;

export type FeatureFlag = (typeof features)[number];

const featureFlagState = valtioPersist('sd-featureFlags', {
	enabled: [] as FeatureFlag[]
});

export function useFeatureFlags() {
	return useSnapshot(featureFlagState);
}

export function useFeatureFlag(flag: FeatureFlag | FeatureFlag[]) {
	useSnapshot(featureFlagState); // Rerender on change
	return Array.isArray(flag) ? flag.every((f) => isEnabled(f)) : isEnabled(flag);
}

export function useOnFeatureFlagsChange(callback: (flags: FeatureFlag[]) => void) {
	useEffect(() => subscribe(featureFlagState, () => callback(featureFlagState.enabled)));
}

export const isEnabled = (flag: FeatureFlag) =>
	featureFlagState.enabled.find((ff) => flag === ff) !== undefined;

export function toggleFeatureFlag(flags: FeatureFlag | FeatureFlag[]) {
	if (!Array.isArray(flags)) {
		flags = [flags];
	}
	flags.forEach((f) => {
		if (!featureFlagState.enabled.find((ff) => f === ff)) {
			if (f === 'p2pPairing') {
				alert(
					'Pairing will render your database broken and it WILL need to be reset! Use at your own risk!'
				);
			} else if (f === 'backups') {
				alert(
					'Backups are done on your live DB without proper Sqlite snapshotting. This will work but it could result in unintended side effects on the backup!'
				);
			}

			featureFlagState.enabled.push(f);
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
	// @ts-expect-error
	return (props) => {
		const enabled = useFeatureFlag(flag);
		// eslint-disable-next-line react-hooks/rules-of-hooks
		return enabled ? <Component /> : fallback;
	};
}
