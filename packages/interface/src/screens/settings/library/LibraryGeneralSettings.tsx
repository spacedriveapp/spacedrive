import { useBridgeCommand, useBridgeQuery } from '@sd/client';
import { Button, Input } from '@sd/ui';
import React, { useCallback, useEffect, useState } from 'react';
import { useDebounce } from 'use-debounce';

import { InputContainer } from '../../../components/primitive/InputContainer';
import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';
import { useCurrentLibrary } from '../../../hooks/useLibraryState';

export default function LibraryGeneralSettings() {
	const { currentLibrary, libraries, currentLibraryUuid } = useCurrentLibrary();

	const { mutate: editLibrary } = useBridgeCommand('EditLibrary');

	const [name, setName] = useState('');
	const [description, setDescription] = useState('');

	const [nameDebounced] = useDebounce(name, 500);
	const [descriptionDebounced] = useDebounce(description, 500);

	useEffect(() => {
		if (currentLibrary) {
			const { name, description } = currentLibrary.config;
			// currentLibrary must be loaded, name must not be empty, and must be different from the current
			if (nameDebounced && (nameDebounced !== name || descriptionDebounced !== description)) {
				editLibrary({
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
			<InputContainer title="Library Name" description="Configure the name of your library.">
				<div className="flex flex-row mt-1 space-x-2">
					<Input
						value={name}
						onChange={(e) => setName(e.target.value)}
						className="flex-grow"
						defaultValue="My Default Library"
					/>
				</div>
			</InputContainer>
			<InputContainer
				title="Library Description"
				// description="Add a short description about this library, what is it for?"
			>
				<div className="flex mt-1">
					<Input
						value={description}
						onChange={(e) => setDescription(e.target.value)}
						className="flex-grow"
						placeholder="Write something about this library"
					/>
				</div>
			</InputContainer>
		</SettingsContainer>
	);
}
