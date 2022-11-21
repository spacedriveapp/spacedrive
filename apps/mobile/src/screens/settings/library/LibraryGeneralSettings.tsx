import { useBridgeMutation, useCurrentLibrary } from '@sd/client';
import React from 'react';
import { Controller, useForm } from 'react-hook-form';
import { Text, View } from 'react-native';
import { Input, SwitchInput } from '~/components/primitive/Input';
import { useAutoForm } from '~/hooks/useAutoForm';
import tw from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/SettingsNavigator';

type LibraryFormData = {
	name: string;
	description: string;
};

const LibraryGeneralSettingsScreen = ({
	navigation
}: SettingsStackScreenProps<'LibraryGeneralSettings'>) => {
	const { library } = useCurrentLibrary();

	const form = useForm<LibraryFormData>({
		defaultValues: { name: library.config.name, description: library.config.description }
	});

	const { mutate: editLibrary } = useBridgeMutation('library.edit');

	useAutoForm(form, (value) => {
		editLibrary({ description: value.description, name: value.name, id: library.uuid });
		console.log('Updated', value);
		// TODO: Show toast
	});

	return (
		<View style={tw`flex-1 p-4`}>
			{/* Name */}
			<Text style={tw`mb-1 text-sm font-medium text-ink-dull ml-1`}>Name</Text>
			<Controller
				name="name"
				control={form.control}
				render={({ field: { onBlur, onChange, value } }) => (
					<Input onBlur={onBlur} onChangeText={onChange} value={value} />
				)}
			/>
			{/* Description */}
			<Text style={tw`mb-1 mt-2 text-sm font-medium text-ink-dull ml-1`}>Description</Text>
			<Controller
				name="description"
				control={form.control}
				render={({ field: { onBlur, onChange, value } }) => (
					<Input onBlur={onBlur} onChangeText={onChange} value={value} />
				)}
			/>
			<View style={tw`mt-8`}>
				{/* Encrypt */}
				<SwitchInput
					value={true}
					title="Encrypt Library"
					description="Enable encryption for this library, this will only encrypt the Spacedrive database, not the files themselves."
				/>
				{/* Export */}
				{/* Delete Library */}
			</View>
		</View>
	);
};

export default LibraryGeneralSettingsScreen;
