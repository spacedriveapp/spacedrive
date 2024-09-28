import { TELEMETRY_LEVEL_PREFERENCES, telemetryState, useTelemetryState } from '@sd/client';
import { Select, SelectOption } from '@sd/ui';
import { useLocale } from '~/hooks';

import { Heading } from '../Layout';
import Setting from '../Setting';

export const Component = () => {
	const { t } = useLocale();

	const { telemetryLevelPreference } = useTelemetryState();

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
				<Select
					value={telemetryLevelPreference}
					onChange={(newValue) => {
						console.log('UPDATE UIPDATE update' + newValue);
						// add "dateFormat" key to localStorage and set it as default date format
						telemetryState.telemetryLevelPreference = newValue;
						console.log('UPDATE UIPDATE update finalize ' + newValue);
					}}
					containerClassName="flex h-[30px] gap-2"
				>
					{TELEMETRY_LEVEL_PREFERENCES.map((format, index) => (
						<SelectOption key={index} value={format}>
							{format}
						</SelectOption>
					))}
				</Select>
			</Setting>
		</>
	);
};
