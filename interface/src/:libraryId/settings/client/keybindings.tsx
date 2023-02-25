import { useState } from 'react';
import { Switch } from '@sd/ui';
import { InputContainer } from '~/components/primitive/InputContainer';
import { Header } from '../Layout';

export default function AppearanceSettings() {
	const [syncWithLibrary, setSyncWithLibrary] = useState(true);
	return (
		<>
			{/* I don't care what you think the "right" way to write "keybinds" is, I simply refuse to refer to it as "keybindings" */}
			<Header title="Keybinds" description="Manage client keybinds" />
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
		</>
	);
}
