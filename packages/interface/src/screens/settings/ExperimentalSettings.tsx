import { Button } from '@sd/ui';
import React from 'react';

import { useStore } from '../../components/device/Stores';
import { Toggle } from '../../components/primitive';
import { InputContainer } from '../../components/primitive/InputContainer';

export default function ExperimentalSettings() {
	// const locations = useBridgeQuery("SysGetLocation")

	const experimental = useStore((state) => state.experimental);

	return (
		<div className="flex flex-col flex-grow max-w-4xl space-y-4">
			{/*<Button size="sm">Add Location</Button>*/}
			<div className="mt-3 mb-3">
				<h1 className="text-2xl font-bold">Experimental</h1>
				<p className="mt-1 text-sm text-gray-400">Experimental features within Spacedrive.</p>
			</div>
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
		</div>
	);
}
