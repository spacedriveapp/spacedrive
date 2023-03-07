import { useSnapshot } from 'valtio';
import { valtioPersist } from '.';

const telemetryState = valtioPersist('sd-telemetryState', {
	shareTelemetry: null as boolean | null
});

export function useTelemetryState() {
	return useSnapshot(telemetryState);
}

export function getTelemetryState() {
	return telemetryState;
}
