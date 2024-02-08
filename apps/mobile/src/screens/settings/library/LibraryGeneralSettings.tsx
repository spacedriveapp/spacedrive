import { Trash } from 'phosphor-react-native';
import React from 'react';
import { Controller } from 'react-hook-form';
import { Alert, Text, View } from 'react-native';
import { z } from 'zod';
import { useBridgeMutation, useLibraryContext, useZodForm } from '@sd/client';
import { Input } from '~/components/form/Input';
import { Switch } from '~/components/form/Switch';
import DeleteLibraryModal from '~/components/modal/confirmModals/DeleteLibraryModal';
import { FakeButton } from '~/components/primitive/Button';
import { Divider } from '~/components/primitive/Divider';
import { SettingsContainer, SettingsTitle } from '~/components/settings/SettingsContainer';
import { SettingsItem } from '~/components/settings/SettingsItem';
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
		<View style={tw`gap-4`}>
			<View style={tw`px-2 mt-4`}>
				<SettingsTitle>Name</SettingsTitle>
				<Controller
					name="name"
					control={form.control}
					render={({ field: { onBlur, onChange, value } }) => (
						<Input onBlur={onBlur} onChangeText={onChange} value={value} />
					)}
				/>
				{/* Description */}
				<SettingsTitle style={tw`mt-4`}>Description</SettingsTitle>
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
				<SettingsContainer description="Enable encryption for this library, this will only encrypt the Spacedrive database, not the files themselves.">
					<SettingsItem title="Encrypt Library" rightArea={<Switch value={true} />} />
				</SettingsContainer>
				{/* Export */}
				<SettingsItem title="Export Library" onPress={() => Alert.alert('TODO')} />
				{/* Delete Library */}
				<DeleteLibraryModal trigger={<Text>Delete</Text>} libraryUuid={library.uuid} />
			</View>
		</View>
	);
};

export default LibraryGeneralSettingsScreen;
