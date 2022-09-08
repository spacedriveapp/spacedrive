import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';

export default function AppearanceSettings() {
	return (
		<SettingsContainer>
			<SettingsHeader title="Appearance" description="Change the look of your client." />
		</SettingsContainer>
	);
}
