import { useSnapshot } from 'valtio';
import { valtioPersist } from './util';

/**
 * Possible Platform types that can be sourced from `usePlatform().platform` or even hardcoded.
 *
 * @remarks
 * The `tauri` platform is renamed to `desktop` for analytic purposes.
 */
export type PlausiblePlatformType = 'web' | 'mobile' | 'desktop' | 'unknown';

type TelemetryState = {
	shareTelemetry: boolean;
	platform: PlausiblePlatformType;
};

export const telemetryStore = valtioPersist<TelemetryState>('sd-telemetryStore', {
	shareTelemetry: false, // false by default, so functions cannot accidentally send data if the user has not decided
	platform: 'unknown'
});

export function useTelemetryState() {
	return useSnapshot(telemetryStore);
}
