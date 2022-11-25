import { useBridgeQuery } from '@sd/client';

import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';

export default function AboutSpacedrive() {
	const buildInfo = useBridgeQuery(['buildInfo']);

	return (
		<SettingsContainer>
			<SettingsHeader title="About Spacedrive" description="The file manager from the future." />

			<h1 className="!m-0 text-sm">
				Build: v{buildInfo.data?.version || '-.-.-'} - {buildInfo.data?.commit || 'dev'}
			</h1>
		</SettingsContainer>
	);
}
