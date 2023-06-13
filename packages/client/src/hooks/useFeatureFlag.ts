import { useSnapshot } from 'valtio';
import { valtioPersist } from '../lib/valito';

export const features = ['spacedrop', 'p2pPairing'] as const;

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

export const isEnabled = (flag: FeatureFlag) => featureFlagState.enabled.find((ff) => flag === ff);

export function toggleFeatureFlag(flags: FeatureFlag | FeatureFlag[]) {
	if (!Array.isArray(flags)) {
		flags = [flags];
	}
	flags.forEach((f) => {
		if (!featureFlagState.enabled.find((ff) => f === ff)) {
			featureFlagState.enabled.push(f);
		} else {
			featureFlagState.enabled = featureFlagState.enabled.filter((ff) => f !== ff);
		}
	});
}
