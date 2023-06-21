import { Trash } from 'phosphor-react-native';
import React from 'react';
import { Controller } from 'react-hook-form';
import { Alert, View } from 'react-native';
import { useBridgeMutation, useLibraryContext } from '@sd/client';
import { Input } from '~/components/form/Input';
import { Switch } from '~/components/form/Switch';
import DeleteLibraryModal from '~/components/modal/confirm-modals/DeleteLibraryModal';
import { FakeButton } from '~/components/primitive/Button';
import { Divider } from '~/components/primitive/Divider';
import { SettingsContainer, SettingsTitle } from '~/components/settings/SettingsContainer';
import { SettingsItem } from '~/components/settings/SettingsItem';
import { useAutoForm } from '~/hooks/useAutoForm';
import { useZodForm, z } from '~/hooks/useZodForm';
import { tw } from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/SettingsNavigator';

const schema = z.object({ name: z.string(), description: z.string() });

const LibraryGeneralSettingsScreen = ({
	navigation
}: SettingsStackScreenProps<'LibraryGeneralSettings'>) => {
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
			<View style={tw`mt-4 px-2`}>
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
				<SettingsContainer description="This is permanent, your files will not be deleted, only the Spacedrive library.">
					<SettingsItem
						title="Delete Library"
						rightArea={
							<DeleteLibraryModal
								libraryUuid={library.uuid}
								trigger={
									<FakeButton size="sm" variant="danger">
										<Trash color={tw.color('ink')} size={20} />
									</FakeButton>
								}
							/>
						}
					/>
				</SettingsContainer>
			</View>
		</View>
	);
};

export default LibraryGeneralSettingsScreen;
