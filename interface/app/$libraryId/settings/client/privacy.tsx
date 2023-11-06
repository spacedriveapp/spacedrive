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
				toolTipLabel="Learn more about telemetry"
				infoUrl="https://www.spacedrive.com/docs/product/resources/privacy"
				title="Share Additional Telemetry and Usage Data"
				description="Toggle ON to provide developers with detailed usage and telemetry data to enhance the app. Toggle OFF to send only basic data: your activity status, app version, core version, and platform (e.g., mobile, web, or desktop)."
			>
				<Switch
					checked={fullTelemetry}
					onClick={() => (telemetryStore.shareFullTelemetry = !fullTelemetry)}
					size="md"
				/>
			</Setting>
		</>
	);
};
