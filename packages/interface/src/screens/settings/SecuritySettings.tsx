import { Button } from '@sd/ui';
import React from 'react';

import { InputContainer } from '../../components/primitive/InputContainer';

export default function SecuritySettings() {
	return (
		<div className="space-y-4">
			<InputContainer
				title="Vault"
				description="You'll need to set a passphrase to enable the vault."
			>
				<div className="flex flex-row">
					<Button variant="primary">Enable Vault</Button>
					{/*<Input className="flex-grow" value="jeff" placeholder="/users/jamie/Desktop" />*/}
				</div>
			</InputContainer>
		</div>
	);
}
