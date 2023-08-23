import { telemetryStore, useTelemetryState } from '@sd/client';
import { Switch } from '@sd/ui';
import { Heading } from '../Layout';
import Setting from '../Setting';

export const Component = () => {
	const fullTelemetry = useTelemetryState().shareFullTelemetry;

	return (
		<>
			<Heading title="Privacy" description="" />
			<Setting
				mini
				title="Share Additional Telemetry and Usage Data"
				description="Enable to share extra usage information and telemetry with developers in order to further improve the app.
				If disabled, the only data sent is that you are an active user, which version of the app and core you're using, and which platform you're on
				(e.g. mobile, web or desktop)."
			>
				<Switch
					checked={fullTelemetry}
					onClick={() => (telemetryStore.shareFullTelemetry = !fullTelemetry)}
					className="m-2 ml-4"
					size="md"
				/>
			</Setting>
		</>
	);
};
