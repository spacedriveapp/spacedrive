import React from 'react';

import { Toggle } from '@sd/ui';
import { InputContainer } from '@sd/ui';
import { SettingsContainer } from '../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../components/settings/SettingsHeader';

// type LibrarySecurity = 'public' | 'password' | 'vault';

export default function LibrarySettings() {
	// const locations = useBridgeQuery("SysGetLocation")
	const [encryptOnCloud, setEncryptOnCloud] = React.useState<boolean>(false);

	return (
		<SettingsContainer>
			{/* <Button size="sm">Add Location</Button> */}
			<SettingsHeader
				title="Library database"
				description="The database contains all library data and file metadata."
			/>
			<InputContainer
				mini
				title="Encrypt on cloud"
				description="Enable if library contains sensitive data and should not be synced to the cloud without full encryption."
			>
				<div className="flex items-center h-full pl-10">
					<Toggle value={encryptOnCloud} onChange={setEncryptOnCloud} size={'sm'} />
				</div>
			</InputContainer>
		</SettingsContainer>
	);
}
