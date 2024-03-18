import { telemetryState, useTelemetryState } from '@sd/client';
import { Switch } from '@sd/ui';
import { useLocale } from '~/hooks';

import { Heading } from '../Layout';
import Setting from '../Setting';

export const Component = () => {
	const fullTelemetry = useTelemetryState().shareFullTelemetry;

	const { t } = useLocale();

	return (
		<>
			<Heading title={t('privacy')} description="" />
			<Setting
				mini
				toolTipLabel={t('learn_more_about_telemetry')}
				infoUrl="https://www.spacedrive.com/docs/product/resources/privacy"
				title={t('telemetry_title')}
				description={t('telemetry_description')}
			>
				<Switch
					checked={fullTelemetry}
					onClick={() => (telemetryState.shareFullTelemetry = !fullTelemetry)}
					size="md"
				/>
			</Setting>
		</>
	);
};
