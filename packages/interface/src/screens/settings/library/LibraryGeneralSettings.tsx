import { useBridgeCommand, useBridgeQuery } from '@sd/client';
import { Button, Input } from '@sd/ui';
import React, { useCallback, useEffect, useState } from 'react';
import { useDebounce } from 'use-debounce';

import { Toggle } from '../../../components/primitive';
import { InputContainer } from '../../../components/primitive/InputContainer';
import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';
import { useCurrentLibrary } from '../../../hooks/useLibraryState';

export default function LibraryGeneralSettings() {
	const { currentLibrary, libraries, currentLibraryUuid } = useCurrentLibrary();

	const { mutate: editLibrary } = useBridgeCommand('EditLibrary');

	const [name, setName] = useState('');
	const [description, setDescription] = useState('');
	const [encryptLibrary, setEncryptLibrary] = useState(false);

	const [nameDebounced] = useDebounce(name, 500);
	const [descriptionDebounced] = useDebounce(description, 500);

	useEffect(() => {
		if (currentLibrary) {
			const { name, description } = currentLibrary.config;
			// currentLibrary must be loaded, name must not be empty, and must be different from the current
			if (nameDebounced && (nameDebounced !== name || descriptionDebounced !== description)) {
				editLibrary({
					id: currentLibraryUuid!,
					name: nameDebounced,
					description: descriptionDebounced
				});
			}
		}
	}, [nameDebounced, descriptionDebounced]);

	useEffect(() => {
		if (currentLibrary) {
			setName(currentLibrary.config.name);
			setDescription(currentLibrary.config.description);
		}
	}, [libraries]);

	return (
		<SettingsContainer>
			<SettingsHeader
				title="Library Settings"
				description="General settings related to the currently active library."
			/>
			<div className="flex flex-row pb-3 space-x-5">
				<div className="flex flex-col flex-grow ">
					<span className="mt-2 mb-1 text-xs font-semibold text-gray-300">Name</span>
					<Input
						value={name}
						onChange={(e) => setName(e.target.value)}
						defaultValue="My Default Library"
					/>
				</div>
				<div className="flex flex-col flex-grow">
					<span className="mt-2 mb-1 text-xs font-semibold text-gray-300">Description</span>
					<Input
						value={description}
						onChange={(e) => setDescription(e.target.value)}
						placeholder=""
					/>
				</div>
			</div>

			<InputContainer
				mini
				title="Encrypt Library"
				description="Enable encryption for this library, this will only encrypt the Spacedrive database, not the files themselves."
			>
				<div className="flex items-center ml-3">
					<Toggle value={encryptLibrary} onChange={setEncryptLibrary} />
				</div>
			</InputContainer>
			<InputContainer
				title="Delete Library"
				description="This is permanent, your files will not be deleted, only the Spacedrive library."
			>
				<div className="mt-2">
					<Button size="sm" variant="colored" className="bg-red-500 border-red-500">
						Delete Library
					</Button>
				</div>
			</InputContainer>
		</SettingsContainer>
	);
}
