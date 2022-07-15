import React from 'react';

import { Toggle } from '../../../components/primitive';
import { InputContainer } from '../../../components/primitive/InputContainer';
import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';

export default function AboutSpacedrive() {
	return (
		<SettingsContainer>
			<SettingsHeader title="About Spacedrive" description="The file manager from the future." />
		</SettingsContainer>
	);
}
