import React from 'react';

import { useNodeStore } from '../../../components/device/Stores';
import { Toggle } from '../../../components/primitive';
import { InputContainer } from '../../../components/primitive/InputContainer';
import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';

export default function ExperimentalSettings() {
	const { isExperimental, setIsExperimental } = useNodeStore();

	return (
		<SettingsContainer>
			{/* <Button size="sm">Add Location</Button> */}
			<SettingsHeader title="Experimental" description="Experimental features within Spacedrive." />
			<InputContainer
				mini
				title="Debug Menu"
				description="Shows data about Spacedrive such as Jobs, Job History and Client State."
			>
				<div className="flex items-center h-full pl-10">
					<Toggle
						value={isExperimental}
						size={'sm'}
						onChange={(newValue) => {
							setIsExperimental(!isExperimental);
						}}
					/>
				</div>
			</InputContainer>
		</SettingsContainer>
	);
}
