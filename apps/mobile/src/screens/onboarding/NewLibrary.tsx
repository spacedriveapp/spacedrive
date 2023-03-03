import { Controller } from 'react-hook-form';
import { Alert, Image, Text, View } from 'react-native';
import { getOnboardingStore, useOnboardingStore } from '@sd/client';
import { Input } from '~/components/form/Input';
import { Button } from '~/components/primitive/Button';
import { useZodForm, z } from '~/hooks/useZodForm';
import { tw } from '~/lib/tailwind';
import { OnboardingStackScreenProps } from '~/navigation/OnboardingNavigator';
import { OnboardingContainer, OnboardingDescription, OnboardingTitle } from './GetStarted';

const schema = z.object({
	name: z.string().min(1, { message: 'Library name is required' })
});

const NewLibraryScreen = ({ navigation }: OnboardingStackScreenProps<'NewLibrary'>) => {
	const obStore = useOnboardingStore();

	const form = useZodForm({
		schema,
		defaultValues: {
			name: obStore.newLibraryName
		}
	});

	const handleNewLibrary = form.handleSubmit(async (data) => {
		getOnboardingStore().newLibraryName = data.name;
		navigation.navigate('MasterPassword');
	});

	const handleImport = () => {
		Alert.alert('TODO');
	};

	return (
		<OnboardingContainer>
			<Image source={require('@sd/assets/images/Database.png')} style={tw`h-25 w-25`} />
			<OnboardingTitle style={tw`mt-4`}>Create a Library</OnboardingTitle>
			<OnboardingDescription style={tw`mt-4`}>
				Libraries are a secure, on-device database. Your files remain where they are, the Library
				catalogs them and stores all Spacedrive related data.
			</OnboardingDescription>
			<Controller
				name="name"
				control={form.control}
				render={({ field: { onBlur, onChange, value } }) => (
					<Input
						style={tw`m-4 w-full`}
						placeholder='e.g. "James Library"'
						onBlur={onBlur}
						onChangeText={onChange}
						value={value}
					/>
				)}
			/>

			{form.formState.errors.name && (
				<Text style={tw`text-center text-xs font-bold text-red-500`}>
					{form.formState.errors.name.message}
				</Text>
			)}
			<View style={tw`mt-4 flex w-full flex-row items-center justify-center`}>
				<Button variant="accent" onPress={handleNewLibrary}>
					<Text style={tw`text-ink text-center text-base font-medium`}>New Library</Text>
				</Button>
				<Text style={tw`text-ink-faint px-4 text-xs font-bold`}>OR</Text>
				<Button onPress={handleImport} variant="outline">
					<Text style={tw`text-ink text-center text-base font-medium`}>Import Library</Text>
				</Button>
			</View>
		</OnboardingContainer>
	);
};

export default NewLibraryScreen;
