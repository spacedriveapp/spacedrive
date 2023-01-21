import { SettingsHeader } from '~/components/settings/SettingsHeader';
import { SettingsSubPage } from '~/components/settings/SettingsSubPage';

export default function LocationSettings() {
	return (
		<SettingsSubPage>
			<SettingsHeader title="Location" description="Manage database backups." />
		</SettingsSubPage>
	);
}
