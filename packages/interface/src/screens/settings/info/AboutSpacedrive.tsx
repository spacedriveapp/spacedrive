import React from 'react';

import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';

export default function AboutSpacedrive() {
	return (
		<SettingsContainer>
			<SettingsHeader title="About Spacedrive" description="The file manager from the future." />
			<span>Version {}</span>
			<div className="flex flex-col">
				<span className="mb-1 text-sm font-bold">Created by</span>
				<span className="max-w-md text-xs text-gray-400">
					Jamie Pine, Brendan Allan, Oscar Beaumont, Haden Fletcher, Haris Mehrzad Benjamin Akar,
					and many more.
				</span>
			</div>
		</SettingsContainer>
	);
}
