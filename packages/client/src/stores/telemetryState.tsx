import { createMutable } from 'solid-js/store';

import { BuildInfo } from '../core';
import { createPersistedMutable, useSolidStore } from '../solid';

/**
 * Possible Platform types that can be sourced from `usePlatform().platform` or even hardcoded.
 *
 * @remarks
 * The `tauri` platform is renamed to `desktop` for analytic purposes.
 */
export type PlausiblePlatformType = 'web' | 'mobile' | 'desktop' | 'unknown';

type TelemetryState = {
	telemetryLevelPreference: TelemetryLevelPreference;
	platform: PlausiblePlatformType;
	buildInfo: BuildInfo | undefined;
};

export const TELEMETRY_LEVEL_PREFERENCES = [
	'full',
	'minimal',
	'none'
] satisfies TelemetryLevelPreference[];
export type TelemetryLevelPreference = 'full' | 'minimal' | 'none';

export const telemetryState = createPersistedMutable(
	'sd-explorer-layout',
	createMutable<TelemetryState>({
		telemetryLevelPreference: 'none', // no telemetry by default
		platform: 'unknown',
		buildInfo: undefined
	})
);

export function useTelemetryState() {
	return useSolidStore(telemetryState);
}
