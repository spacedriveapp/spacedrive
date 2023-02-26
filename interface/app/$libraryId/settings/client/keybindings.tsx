import { useState } from 'react';
import { Switch } from '@sd/ui';
import { Heading } from '../Layout';
import Setting from '../Setting';

export default function AppearanceSettings() {
	const [syncWithLibrary, setSyncWithLibrary] = useState(true);
	return (
		<>
			{/* I don't care what you think the "right" way to write "keybinds" is, I simply refuse to refer to it as "keybindings" */}
			<Heading title="Keybinds" description="Manage client keybinds" />
			<Setting
				mini
				title="Sync with Library"
				description="If enabled your keybinds will be synced with library, otherwise they will apply only to this client."
			>
				<Switch
					checked={syncWithLibrary}
					onCheckedChange={setSyncWithLibrary}
					className="m-2 ml-4"
				/>
			</Setting>
		</>
	);
}
