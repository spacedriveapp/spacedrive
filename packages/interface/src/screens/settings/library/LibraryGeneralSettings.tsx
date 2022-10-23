import { useBridgeMutation } from '@sd/client';
import { useCurrentLibrary } from '@sd/client';
import { Button, Input, Switch } from '@sd/ui';
import { useCallback, useEffect, useState } from 'react';
import { useForm } from 'react-hook-form';
import { useDebounce } from 'rooks';

import { InputContainer } from '../../../components/primitive/InputContainer';
import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';

export default function LibraryGeneralSettings() {
	const { library, libraries } = useCurrentLibrary();

	const [encryptLibrary, setEncryptLibrary] = useState(false);

	const editLibrary = useBridgeMutation('library.edit');

	const { register, reset, handleSubmit, watch } = useForm({
		defaultValues: {
			name: library?.config.name,
			description: library?.config.description
		}
	});

	// reset form when library changes
	useEffect(() => {
		reset({
			name: library?.config.name,
			description: library?.config.description
		});
		console.log('libraries changed, resetting form', library, libraries);
	}, [libraries, library, reset]);

	const handleEditLibrary = handleSubmit((data) => {
		console.log("updating library's name and description", library?.uuid, data);
		if (library?.uuid) {
			console.log('library.uuid', library.uuid);
			editLibrary.mutate({
				id: library.uuid,
				name: data.name || null,
				description: data.description || null
			});
		}
	});
	// @ts-ignore
	// eslint-disable-next-line react-hooks/exhaustive-deps
	const update = useCallback(useDebounce(handleEditLibrary, 500), []);
	useEffect(() => {
		const subscription = watch(() => update());
		return () => subscription.unsubscribe();
	});

	// const [name, setName] = useState('');
	// const [description, setDescription] = useState('');
	// const [nameDebounced] = useDebounce(name, 500);
	// const [descriptionDebounced] = useDebounce(description, 500);
	// prevent auto update when switching library

	// useEffect(() => {
	// 	if (library) {
	// 		const { name, description } = library.config;
	// 		// currentLibrary must be loaded, name must not be empty, and must be different from the current
	// 		if (nameDebounced && (nameDebounced !== name || descriptionDebounced !== description)) {
	// 			editLibrary({
	// 				id: library.uuid!,
	// 				name: nameDebounced,
	// 				description: descriptionDebounced
	// 			});
	// 		}
	// 	}
	// }, [nameDebounced, descriptionDebounced, library, editLibrary]);

	// useEffect(() => {
	// 	if (library) {
	// 		setName(library.config.name);
	// 		setDescription(library.config.description);
	// 	}
	// }, [libraries, library]);

	// useEffect(() => {
	// 	if (library) {
	// 		setBlockAutoUpdate(true);
	// 		setName(library.config.name);
	// 		setDescription(library.config.description);
	// 	}
	// }, [library]);

	// useEffect(() => {
	// 	if (blockAutoUpdate) setBlockAutoUpdate(false);
	// }, [blockAutoUpdate]);

	return (
		<SettingsContainer>
			<SettingsHeader
				title="Library Settings"
				description="General settings related to the currently active library."
			/>
			<div className="flex flex-row pb-3 space-x-5">
				<div className="flex flex-col flex-grow">
					<span className="mb-1 text-sm font-medium">Name</span>
					<Input {...register('name')} defaultValue="My Default Library" />
				</div>
				<div className="flex flex-col flex-grow">
					<span className="mb-1 text-sm font-medium">Description</span>
					<Input {...register('description')} placeholder="" />
				</div>
			</div>

			<InputContainer
				mini
				title="Encrypt Library"
				description="Enable encryption for this library, this will only encrypt the Spacedrive database, not the files themselves."
			>
				<div className="flex items-center ml-3">
					<Switch checked={encryptLibrary} onCheckedChange={setEncryptLibrary} />
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
