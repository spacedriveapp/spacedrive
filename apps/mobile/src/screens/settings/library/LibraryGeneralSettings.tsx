import { Controller } from 'react-hook-form';
import { Text, View } from 'react-native';
import { TouchableOpacity } from 'react-native-gesture-handler';
import { z } from 'zod';
import { useBridgeMutation, useLibraryContext, useZodForm } from '@sd/client';
import ScreenContainer from '~/components/layout/ScreenContainer';
import DeleteLibraryModal from '~/components/modal/confirmModals/DeleteLibraryModal';
import { Button } from '~/components/primitive/Button';
import { Divider } from '~/components/primitive/Divider';
import { Input } from '~/components/primitive/Input';
import { toast } from '~/components/primitive/Toast';
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
		toast.success('Library updated!');
	});

	return (
		<ScreenContainer scrollview={false} style={tw`justify-start px-6 py-0`}>
			<View style={tw`pt-5`}>
				<SettingsTitle style={tw`mb-1`}>Name</SettingsTitle>
				<Controller
					name="name"
					control={form.control}
					render={({ field: { onBlur, onChange, value } }) => (
						<Input onBlur={onBlur} onChangeText={onChange} value={value} />
					)}
				/>
				<SettingsTitle style={tw`mb-1 mt-4`}>Description</SettingsTitle>
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
					onPress={() => {
						//TODO: Export library
					}}
					buttonTextStyle="font-bold text-ink-dull"
					title="Export Library"
				/>
				{/* Delete Library */}
				<View style={tw`flex-row items-center justify-between`}>
					<View style={tw`w-[73%]`}>
						<Text style={tw`text-sm font-medium text-ink`}>Delete Library</Text>
						<Text style={tw`mt-1 text-xs text-ink-dull`}>
							This is permanent, your files not be deleted, only the Spacedrive
							library.
						</Text>
					</View>
					<DeleteLibraryModal
						trigger={
							<View style={tw`rounded-md border-red-800 bg-red-600 px-3 py-1.5`}>
								<Text style={tw`font-bold text-ink`}>Delete</Text>
							</View>
						}
						libraryUuid={library.uuid}
					/>
				</View>
			</View>
		</ScreenContainer>
	);
};

export default LibraryGeneralSettingsScreen;
