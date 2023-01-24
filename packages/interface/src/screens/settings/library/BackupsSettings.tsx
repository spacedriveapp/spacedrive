import { SettingsContainer } from '~/components/settings/SettingsContainer';
import { SettingsHeader } from '~/components/settings/SettingsHeader';

export default function NodesSettings() {
	return (
		<SettingsContainer>
			<SettingsHeader title="Backups" description="Manage database backups." />
		</SettingsContainer>
	);
}
