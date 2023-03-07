import { useSnapshot } from 'valtio';
import { valtioPersist } from '.';

export const telemetryStore = valtioPersist('sd-telemetryStore', {
	shareTelemetry: false // false by default, so functions cannot accidentally send data if the user has not decided. could also be undefined?
});

export function useTelemetryState() {
	return useSnapshot(telemetryStore);
}
