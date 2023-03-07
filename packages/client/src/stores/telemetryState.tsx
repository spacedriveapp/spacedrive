import { useSnapshot } from 'valtio';
import { valtioPersist } from '.';

const telemetryState = valtioPersist('sd-telemetryState', {
	shareTelemetry: null as boolean | null
});

export const useTelemetryState = () => {
	return useSnapshot(telemetryState);
};

export const getTelemetryState = () => {
	return telemetryState;
};
