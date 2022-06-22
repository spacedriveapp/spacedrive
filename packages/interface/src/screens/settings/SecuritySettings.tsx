import { Button } from '@sd/ui';
import React from 'react';

import { InputContainer } from '../../../../ui/src/InputContainer';
import { SettingsContainer } from '../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../components/settings/SettingsHeader';

export default function SecuritySettings() {
	return (
		<SettingsContainer>
			<SettingsHeader title="Security" description="Keep your client safe." />
			<InputContainer
				title="Vault"
				description="You'll need to set a passphrase to enable the vault."
			>
				<div className="flex flex-row">
					<Button variant="primary">Enable Vault</Button>
					{/*<Input className="flex-grow" value="jeff" placeholder="/users/jamie/Desktop" />*/}
				</div>
			</InputContainer>
		</SettingsContainer>
	);
}
