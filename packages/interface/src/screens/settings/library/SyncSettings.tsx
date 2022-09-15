import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';

export default function SyncSettings() {
	return (
		<SettingsContainer>
			<SettingsHeader title="Sync" description="Manage how Spacedrive syncs." />
		</SettingsContainer>
	);
}
