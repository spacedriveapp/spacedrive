import React from 'react';

import { useStore } from '../../components/device/Stores';
import { Toggle } from '../../components/primitive';
import { InputContainer } from '../../components/primitive/InputContainer';
import { SettingsContainer } from '../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../components/settings/SettingsHeader';

export default function ExperimentalSettings() {
	// const locations = useBridgeQuery("SysGetLocation")

	const experimental = useStore((state) => state.experimental);

	return (
		<SettingsContainer>
			{/*<Button size="sm">Add Location</Button>*/}
			<SettingsHeader title="Experimental" description="Experimental features within Spacedrive." />
			<InputContainer
				mini
				title="Debug Menu"
				description="Shows data about Spacedrive such as Jobs, Job History and Client State."
			>
				<div className="flex items-center h-full pl-10">
					<Toggle
						value={experimental}
						size={'sm'}
						onChange={(newValue) => {
							useStore.setState({
								experimental: newValue
							});
						}}
					/>
				</div>
			</InputContainer>
		</SettingsContainer>
	);
}
