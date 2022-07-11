import { Button } from '@sd/ui';
import React from 'react';

import { InputContainer } from '../../../components/primitive/InputContainer';
import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';

export default function SecuritySettings() {
	return (
		<SettingsContainer>
			<SettingsHeader title="Security" description="Keep your client safe." />
		</SettingsContainer>
	);
}
