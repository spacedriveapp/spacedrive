import { useSnapshot } from 'valtio';
import { proxySet } from 'valtio/utils';
import { valtioPersist } from '../lib/valito';

const features = ['spacedrop', 'p2pPairing'] as const;

export type FeatureFlag = (typeof features)[number];

const featureFlagState = valtioPersist('sd-featureFlags', {
	enabled: proxySet<FeatureFlag>()
});

export function useFeatureFlag(flag: FeatureFlag | FeatureFlag[]) {
	const state = useSnapshot(featureFlagState);
	return Array.isArray(flag) ? flag.every((f) => state.enabled.has(f)) : state.enabled.has(flag);
}

export function isFeatureEnabled(flag: FeatureFlag | FeatureFlag[]) {
	return Array.isArray(flag)
		? flag.every((f) => featureFlagState.enabled.has(f))
		: featureFlagState.enabled.has(flag);
}

export function enableFeatureFlag(flags: FeatureFlag | FeatureFlag[]) {
	if (!Array.isArray(flags)) {
		flags = [flags];
	}
	flags.forEach((f) => featureFlagState.enabled.add(f));
}
