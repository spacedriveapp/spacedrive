import React from 'react';

import { Toggle } from '../../../components/primitive';
import { InputContainer } from '../../../components/primitive/InputContainer';
import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';

export default function PrivacySettings() {
	return (
		<SettingsContainer>
			<SettingsHeader title="Privacy" description="How Spacedrive handles your data" />
		</SettingsContainer>
	);
}
