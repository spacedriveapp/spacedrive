import { useBridgeMutation } from '@sd/client';
import { useCurrentLibrary } from '@sd/client';
import { Button, Input } from '@sd/ui';
import { useEffect, useState } from 'react';
import { useForm } from 'react-hook-form';
import { useDebouncedCallback } from 'use-debounce';

import { Toggle } from '../../../components/primitive';
import { InputContainer } from '../../../components/primitive/InputContainer';
import { SettingsContainer } from '../../../components/settings/SettingsContainer';
import { SettingsHeader } from '../../../components/settings/SettingsHeader';

export default function LibraryGeneralSettings() {
	const { library } = useCurrentLibrary();
	const { mutate: editLibrary } = useBridgeMutation('library.edit');
	const debounced = useDebouncedCallback((value) => {
		editLibrary({
			id: library!.uuid,
			name: value.name,
			description: value.description
		});
	}, 500);
	const { register, watch } = useForm({
		defaultValues: {
			name: library?.config.name,
			description: library?.config.description,
			encryptLibrary: false // TODO: From backend
		}
	});

	watch(debounced); // Listen for form changes

	// This forces the debounce to run when the component is unmounted
	useEffect(() => () => debounced.flush(), [debounced]);

	return (
		<SettingsContainer>
			<SettingsHeader
				title="Library Settings"
				description="General settings related to the currently active library."
			/>
			<div className="flex flex-row pb-3 space-x-5">
				<div className="flex flex-col flex-grow">
					<span className="mb-1 text-sm font-medium text-gray-700 dark:text-gray-100">Name</span>
					<Input {...register('name', { required: true })} defaultValue="My Default Library" />
				</div>
				<div className="flex flex-col flex-grow">
					<span className="mb-1 text-sm font-medium text-gray-700 dark:text-gray-100">
						Description
					</span>
					<Input {...register('description')} placeholder="" />
				</div>
			</div>

			<InputContainer
				mini
				title="Encrypt Library"
				description="Enable encryption for this library, this will only encrypt the Spacedrive database, not the files themselves."
			>
				<div className="flex items-center ml-3">
					<Toggle {...register('encryptLibrary')} />
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
