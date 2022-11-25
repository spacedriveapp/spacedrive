import { useBridgeMutation, useCurrentLibrary } from '@sd/client';
import React from 'react';
import { Controller, useForm } from 'react-hook-form';
import { Alert, Text, View } from 'react-native';
import Card from '~/components/layout/Card';
import { Button } from '~/components/primitive/Button';
import Divider from '~/components/primitive/Divider';
import { Input } from '~/components/primitive/Input';
import { Switch } from '~/components/primitive/Switch';
import { SettingsContainer } from '~/components/settings/SettingsContainer';
import { SettingsItem } from '~/components/settings/SettingsItem';
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
		<View>
			<Card style={tw`m-4`}>
				<Text style={tw`font-semibold text-ink`}>Current Library</Text>
				<Divider style={tw`my-2`} />
				<Text style={tw`mb-1 text-xs font-medium text-ink-dull ml-1`}>Name</Text>
				<Controller
					name="name"
					control={form.control}
					render={({ field: { onBlur, onChange, value } }) => (
						<Input onBlur={onBlur} onChangeText={onChange} value={value} />
					)}
				/>
				{/* Description */}
				<Text style={tw`mb-1 text-xs font-medium text-ink-dull ml-1 mt-2`}>Description</Text>
				<Controller
					name="description"
					control={form.control}
					render={({ field: { onBlur, onChange, value } }) => (
						<Input onBlur={onBlur} onChangeText={onChange} value={value} />
					)}
				/>
			</Card>
			{/* Encrypt */}
			<View style={tw`mt-6`} />
			<SettingsContainer description="Enable encryption for this library, this will only encrypt the Spacedrive database, not the files themselves.">
				<SettingsItem title="Encrypt Library" rightArea={<Switch value={true} />} />
			</SettingsContainer>
			<View style={tw`mt-6`} />
			{/* Export */}
			<SettingsItem title="Export Library" onPress={() => Alert.alert('TODO')} />
			<View style={tw`mt-4`} />
			{/* Delete Library
			TODO: Open delete library dialog here, but do handle library switching
			And what happens if there is no library set ?
			*/}
			<SettingsContainer description="This is permanent, your files will not be deleted, only the Spacedrive library.">
				<SettingsItem
					title="Delete Library"
					rightArea={
						<Button size="md" variant="danger" onPress={() => Alert.alert('TODO')}>
							<Text style={tw`text-ink text-sm font-medium`}>Delete</Text>
						</Button>
					}
				/>
			</SettingsContainer>
		</View>
	);
};

export default LibraryGeneralSettingsScreen;
