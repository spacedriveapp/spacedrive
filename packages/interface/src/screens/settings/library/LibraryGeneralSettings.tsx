import { useBridgeCommand, useBridgeQuery } from '@sd/client';
import { Button, Input } from '@sd/ui';
import React, { useEffect, useState } from 'react';

import { InputContainer } from '../../../components/primitive/InputContainer';
import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';
import { useCurrentLibrary, useLibraryState } from '../../../hooks/useLibraryState';

export default function LibraryGeneralSettings() {
	const { currentLibrary, libraries, currentLibraryUuid } = useCurrentLibrary();

	const { mutate: editLibrary } = useBridgeCommand('EditLibrary');

	const [name, setName] = useState('');

	useEffect(() => {
		if (currentLibraryUuid) {
			const library = libraries?.find((library) => library.uuid === currentLibraryUuid);
			if (library) {
				setName(library.config.name);
			}
		}
	}, [libraries]);

	return (
		<SettingsContainer>
			<SettingsHeader
				title="Library Settings"
				description="General settings related to the currently active library."
			/>
			<InputContainer title="Library Name" description="Configure the name of your library.">
				<div className="flex flex-row mt-1 space-x-2">
					<Input
						value={name}
						onChange={(e) => setName(e.target.value)}
						className="flex-grow"
						defaultValue="My Default Library"
					/>
					<Button
						variant="primary"
						onClick={() =>
							editLibrary({
								name,
								description: ''
							})
						}
					>
						Save
					</Button>
				</div>
			</InputContainer>
		</SettingsContainer>
	);
}
