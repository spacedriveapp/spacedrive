import React from 'react';

import { Toggle } from '../../../components/primitive';
import { InputContainer } from '../../../components/primitive/InputContainer';
import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';

export default function AppearanceSettings() {
	return (
		<SettingsContainer>
			<SettingsHeader title="Keybindings" description="Manage client keybindings" />
			<InputContainer
				mini
				title="Sync with Library"
				description="If enabled your keybindings will be synced with library, otherwise they will apply only to this client."
			>
				<Toggle value />
			</InputContainer>
		</SettingsContainer>
	);
}
