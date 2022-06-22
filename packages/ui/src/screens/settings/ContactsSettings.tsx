import React from 'react';

import { SettingsContainer } from '../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../components/settings/SettingsHeader';

export default function ContactsSettings() {
	return (
		<SettingsContainer>
			<SettingsHeader title="Contacts" description="Manage your contacts in Spacedrive." />
		</SettingsContainer>
	);
}
