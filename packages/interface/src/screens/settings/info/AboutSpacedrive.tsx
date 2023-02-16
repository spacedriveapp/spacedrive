import Logo from '@sd/assets/images/logo.png';
import { useBridgeQuery } from '@sd/client';
import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';

export default function AboutSpacedrive() {
	const buildInfo = useBridgeQuery(['buildInfo']);

	return (
		<SettingsContainer>
			<SettingsHeader
				title="Spacedrive"
				description={
					<div className="flex flex-col">
						<span>The file manager from the future.</span>
						<span className="text-ink-faint/80 mt-2 text-xs">
							v{buildInfo.data?.version || '-.-.-'} - {buildInfo.data?.commit || 'dev'}
						</span>
					</div>
				}
			>
				<img src={Logo} className="mr-8 w-[88px]" />
			</SettingsHeader>
		</SettingsContainer>
	);
}
