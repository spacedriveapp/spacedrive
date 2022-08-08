import React from 'react';

import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';

export default function Changelog() {
	return (
		<SettingsContainer>
			<SettingsHeader title="Changelog" description="See what cool new features we're making" />
		</SettingsContainer>
	);
}
