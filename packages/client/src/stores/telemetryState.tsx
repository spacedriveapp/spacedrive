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

type TelemetryStateV0 = {
	shareFullTelemetry: boolean;
	platform: PlausiblePlatformType;
	buildInfo: BuildInfo | undefined;
};

type TelemetryStateV1 = {
	telemetryLevelPreference: TelemetryLevelPreference;
	platform: PlausiblePlatformType;
	buildInfo: BuildInfo | undefined;
	stateVersion: 1;
};

export type TelemetryLevelPreference = 'full' | 'minimal' | 'none';
export const TELEMETRY_LEVEL_PREFERENCES = [
	'full',
	'minimal',
	'none'
] satisfies TelemetryLevelPreference[];

const DEFAULT_TELEMETRY_STATE: TelemetryStateV1 = {
	telemetryLevelPreference: 'none',
	platform: 'unknown',
	buildInfo: undefined,
	stateVersion: 1
};

export const telemetryState = createPersistedMutable<TelemetryStateV1>(
	'sd-explorer-layout',
	createMutable<TelemetryStateV1>(DEFAULT_TELEMETRY_STATE),
	{
		onLoad(value: unknown): TelemetryStateV1 {
			if (isV1State(value)) return value;
			if (isV0State(value)) return migrateV0ToV1(value);

			// If the value is neither v0 nor v1, return the default state
			return DEFAULT_TELEMETRY_STATE;
		}
	}
);

function isV0State(value: unknown): value is TelemetryStateV0 {
	return (
		typeof value === 'object' &&
		value !== null &&
		'shareFullTelemetry' in value &&
		'platform' in value &&
		'buildInfo' in value
	);
}

function isV1State(value: unknown): value is TelemetryStateV1 {
	return (
		typeof value === 'object' &&
		value !== null &&
		'telemetryLevelPreference' in value &&
		'platform' in value &&
		'buildInfo' in value &&
		'stateVersion' in value &&
		value.stateVersion === 1
	);
}

function migrateV0ToV1(value: TelemetryStateV0): TelemetryStateV1 {
	return {
		...DEFAULT_TELEMETRY_STATE,
		telemetryLevelPreference: value.shareFullTelemetry ? 'full' : 'minimal'
	};
}

export function useTelemetryState() {
	return useSolidStore(telemetryState);
}
