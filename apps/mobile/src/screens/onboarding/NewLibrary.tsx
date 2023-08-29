import { Database } from '@sd/assets/icons';
import { Controller } from 'react-hook-form';
import { Alert, Image, Text, View } from 'react-native';
import { Input } from '~/components/form/Input';
import { Button } from '~/components/primitive/Button';
import { tw } from '~/lib/tailwind';
import { OnboardingStackScreenProps } from '~/navigation/OnboardingNavigator';
import { OnboardingContainer, OnboardingDescription, OnboardingTitle } from './GetStarted';
import { useOnboardingContext } from './context';

const NewLibraryScreen = ({ navigation }: OnboardingStackScreenProps<'NewLibrary'>) => {
	const form = useOnboardingContext().forms.useForm('NewLibrary');

	const handleNewLibrary = form.handleSubmit(() => navigation.navigate('Privacy'));

	const handleImport = () => {
		Alert.alert('TODO');
	};

	return (
		<OnboardingContainer>
			<Image source={Database} style={tw`h-25 w-25`} />
			<OnboardingTitle style={tw`mt-4`}>Create a Library</OnboardingTitle>
			<View style={tw`w-full px-4`}>
				<OnboardingDescription style={tw`mt-4`}>
					Libraries are a secure, on-device database. Your files remain where they are,
					the Library catalogs them and stores all Spacedrive related data.
				</OnboardingDescription>
				<Controller
					name="name"
					control={form.control}
					render={({ field: { onBlur, onChange, value } }) => (
						<Input
							testID="library-name"
							style={tw`my-3`}
							placeholder='e.g. "James Library"'
							onBlur={onBlur}
							onChangeText={onChange}
							value={value}
						/>
					)}
				/>
			</View>

			{form.formState.errors.name && (
				<Text style={tw`text-center text-xs font-bold text-red-500`}>
					{form.formState.errors.name.message}
				</Text>
			)}
			<View style={tw`mt-4 flex w-full flex-row items-center justify-center`}>
				<Button variant="accent" onPress={handleNewLibrary}>
					<Text style={tw`text-center font-medium text-ink`}>New Library</Text>
				</Button>
				<Text style={tw`px-4 text-xs font-bold text-ink-faint`}>OR</Text>
				<Button onPress={handleImport} variant="outline">
					<Text style={tw`text-center font-medium text-ink`}>Import Library</Text>
				</Button>
			</View>
		</OnboardingContainer>
	);
};

export default NewLibraryScreen;
