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
	shareFullTelemetry: boolean;
	platform: PlausiblePlatformType;
	buildInfo: BuildInfo | undefined;
};

export const telemetryState = createPersistedMutable(
	'sd-explorer-layout',
	createMutable<TelemetryState>({
		shareFullTelemetry: false, // false by default
		platform: 'unknown',
		buildInfo: undefined
	})
);

export function useTelemetryState() {
	return useSolidStore(telemetryState);
}
