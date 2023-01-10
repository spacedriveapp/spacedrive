import { Switch } from '@sd/ui';
import { useState } from 'react';

import { InputContainer } from '~/components/primitive/InputContainer';
import { SettingsContainer } from '~/components/settings/SettingsContainer';
import { SettingsHeader } from '~/components/settings/SettingsHeader';

export default function AppearanceSettings() {
	const [syncWithLibrary, setSyncWithLibrary] = useState(true);
	return (
		<SettingsContainer>
			{/* I don't care what you think the "right" way to write "keybinds" is, I simply refuse to refer to it as "keybindings" */}
			<SettingsHeader title="Keybinds" description="Manage client keybinds" />
			<InputContainer
				mini
				title="Sync with Library"
				description="If enabled your keybinds will be synced with library, otherwise they will apply only to this client."
			>
				<Switch
					checked={syncWithLibrary}
					onCheckedChange={setSyncWithLibrary}
					className="m-2 ml-4"
				/>
			</InputContainer>
		</SettingsContainer>
	);
}
