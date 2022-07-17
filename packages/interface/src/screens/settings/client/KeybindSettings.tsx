import React from 'react';

import { Toggle } from '../../../components/primitive';
import { InputContainer } from '../../../components/primitive/InputContainer';
import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';

export default function AppearanceSettings() {
	return (
		<SettingsContainer>
			<SettingsHeader title="Keybinds" description="Manage client keybinds" />
			<InputContainer
				mini
				title="Sync with Library"
				description="If enabled your keybinds will be synced with library, otherwise they will apply only to this client."
			>
				<Toggle value />
			</InputContainer>
		</SettingsContainer>
	);
}
