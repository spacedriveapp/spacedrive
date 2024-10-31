import { TelemetryLevelPreference, telemetryState, useTelemetryState } from '@sd/client';
import { Select, SelectOption } from '@sd/ui';
import i18n from '~/app/I18n';
import { useLocale } from '~/hooks';

import { Heading } from '../Layout';
import Setting from '../Setting';

const telemetryPreferenceOptions = [
	{ value: 'full', label: i18n.t('telemetry_share_anonymous_short') },
	{ value: 'minimal', label: i18n.t('telemetry_share_minimal_short') },
	{ value: 'none', label: i18n.t('telemetry_share_none_short') }
] satisfies { value: TelemetryLevelPreference; label: string }[];

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
						// add "dateFormat" key to localStorage and set it as default date format
						telemetryState.telemetryLevelPreference = newValue;
					}}
					containerClassName="flex h-[30px] gap-2"
				>
					{telemetryPreferenceOptions.map((format, index) => (
						<SelectOption key={index} value={format.value}>
							{format.label}
						</SelectOption>
					))}
				</Select>
			</Setting>
		</>
	);
};
