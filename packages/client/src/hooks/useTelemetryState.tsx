import { useSnapshot } from 'valtio';

import { BuildInfo } from '../core';
import { valtioPersist } from '../lib';

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

export const telemetryStore = valtioPersist<TelemetryState>('sd-telemetryStore', {
	shareFullTelemetry: false, // false by default
	platform: 'unknown',
	buildInfo: undefined
});

export function useTelemetryState() {
	return useSnapshot(telemetryStore);
}
