import React from 'react';
import { Controller } from 'react-hook-form';
import { Text, View } from 'react-native';
import { z } from 'zod';
import { useBridgeMutation, useLibraryContext, useZodForm } from '@sd/client';
import { Input } from '~/components/form/Input';
import ScreenContainer from '~/components/layout/ScreenContainer';
import DeleteLibraryModal from '~/components/modal/confirmModals/DeleteLibraryModal';
import { Divider } from '~/components/primitive/Divider';
import SettingsButton from '~/components/settings/SettingsButton';
import { SettingsTitle } from '~/components/settings/SettingsContainer';
import SettingsToggle from '~/components/settings/SettingsToggle';
import { useAutoForm } from '~/hooks/useAutoForm';
import { tw } from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/tabs/SettingsStack';

const schema = z.object({ name: z.string(), description: z.string() });

const LibraryGeneralSettingsScreen = (_: SettingsStackScreenProps<'LibraryGeneralSettings'>) => {
	const { library } = useLibraryContext();

	const form = useZodForm({
		schema,
		defaultValues: {
			name: library.config.name,
			description: library.config.description || undefined
		}
	});

	const { mutate: editLibrary } = useBridgeMutation('library.edit');

	useAutoForm(form, (value) => {
		editLibrary({ description: value.description, name: value.name, id: library.uuid });
		// console.log('Updated', value);
		// TODO: Show toast
	});

	return (
		<ScreenContainer scrollview={false} style={tw`justify-start py-0 px-7`}>
			<View style={tw`pt-5`}>
				<SettingsTitle style={tw`mb-1`}>Name</SettingsTitle>
				<Controller
					name="name"
					control={form.control}
					render={({ field: { onBlur, onChange, value } }) => (
						<Input onBlur={onBlur} onChangeText={onChange} value={value} />
					)}
				/>
				<SettingsTitle style={tw`mt-4 mb-1`}>Description</SettingsTitle>
				<Controller
					name="description"
					control={form.control}
					render={({ field: { onBlur, onChange, value } }) => (
						<Input onBlur={onBlur} onChangeText={onChange} value={value} />
					)}
				/>
			</View>
			<Divider />
			<View style={tw`gap-y-6`}>
				{/* Encrypt */}
				<SettingsToggle
					onEnabledChange={(enabled) => {
						//TODO: Enable encryption
					}}
					title="Encrypt Library"
					description="Enable encryption for this library, this will only encrypt the Spacedrive database, not the files themselves."
				/>
				{/* Export */}
				<SettingsButton
					description="Export this library to a file."
					buttonText="Export"
					buttonPress={() => {
						//TODO: Export library
					}}
					buttonTextStyle="font-bold text-ink-dull"
					title="Export Library"
				/>
				{/* Delete Library */}
				<DeleteLibraryModal trigger={<Text>Delete</Text>} libraryUuid={library.uuid} />
			</View>
		</ScreenContainer>
	);
};

export default LibraryGeneralSettingsScreen;
