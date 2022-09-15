import { useBridgeMutation } from '@sd/client';
import { useCurrentLibrary } from '@sd/client';
import { Button, Input } from '@sd/ui';
import { useEffect, useState } from 'react';
import { useDebounce } from 'use-debounce';

import { Toggle } from '../../../components/primitive';
import { InputContainer } from '../../../components/primitive/InputContainer';
import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';

export default function LibraryGeneralSettings() {
	const { library, libraries } = useCurrentLibrary();

	const { mutate: editLibrary } = useBridgeMutation('library.edit');

	const [name, setName] = useState('');
	const [description, setDescription] = useState('');
	const [encryptLibrary, setEncryptLibrary] = useState(false);
	// prevent auto update when switching library
	const [blockAutoUpdate, setBlockAutoUpdate] = useState(false);

	const [nameDebounced] = useDebounce(name, 500);
	const [descriptionDebounced] = useDebounce(description, 500);

	useEffect(() => {
		if (library) {
			const { name, description } = library.config;
			// currentLibrary must be loaded, name must not be empty, and must be different from the current
			if (nameDebounced && (nameDebounced !== name || descriptionDebounced !== description)) {
				editLibrary({
					id: library.uuid!,
					name: nameDebounced,
					description: descriptionDebounced
				});
			}
		}
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [nameDebounced, descriptionDebounced]);

	useEffect(() => {
		if (library) {
			setName(library.config.name);
			setDescription(library.config.description);
		}
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [libraries]);

	useEffect(() => {
		if (library) {
			setBlockAutoUpdate(true);
			setName(library.config.name);
			setDescription(library.config.description);
		}
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [library]);

	useEffect(() => {
		if (blockAutoUpdate) setBlockAutoUpdate(false);
	}, [blockAutoUpdate]);

	return (
		<SettingsContainer>
			<SettingsHeader
				title="Library Settings"
				description="General settings related to the currently active library."
			/>
			<div className="flex flex-row pb-3 space-x-5">
				<div className="flex flex-col flex-grow">
					<span className="mb-1 text-sm font-medium text-gray-700 dark:text-gray-100">Name</span>
					<Input
						value={name}
						onChange={(e) => setName(e.target.value)}
						defaultValue="My Default Library"
					/>
				</div>
				<div className="flex flex-col flex-grow">
					<span className="mb-1 text-sm font-medium text-gray-700 dark:text-gray-100">
						Description
					</span>
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
			<InputContainer mini title="Export Library" description="Export this library to a file.">
				<div className="mt-2">
					<Button size="sm" variant="gray">
						Export
					</Button>
				</div>
			</InputContainer>
			<InputContainer
				mini
				title="Delete Library"
				description="This is permanent, your files will not be deleted, only the Spacedrive library."
			>
				<div className="mt-2">
					<Button size="sm" variant="colored" className="bg-red-500 border-red-500">
						Delete
					</Button>
				</div>
			</InputContainer>
		</SettingsContainer>
	);
}
