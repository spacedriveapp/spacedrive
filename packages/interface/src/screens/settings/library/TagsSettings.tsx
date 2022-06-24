import React from 'react';

import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';

export default function TagsSettings() {
	return (
		<SettingsContainer>
			<SettingsHeader title="Tags" description="Manage your tags." />
		</SettingsContainer>
	);
}
