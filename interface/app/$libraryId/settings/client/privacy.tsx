import { Switch } from '@sd/ui';
import { getTelemetryState, useTelemetryState } from '~/../packages/client/src';
import { Heading } from '../Layout';
import Setting from '../Setting';

export default function PrivacySettings() {
	const shareTelemetry = useTelemetryState().shareTelemetry;
	const telemetryState = getTelemetryState();

	return (
		<>
			<Heading title="Privacy" description="" />
			<Setting
				mini
				title="Share Usage Data"
				description="Share anonymous usage data to help us improve the app."
			>
				<Switch
					checked={shareTelemetry ?? undefined}
					onCheckedChange={(e) => (telemetryState.shareTelemetry = e)}
					className="m-2 ml-4"
				/>
			</Setting>
		</>
	);
}
