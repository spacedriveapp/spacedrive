import * as Haptics from 'expo-haptics';
import { Controller } from 'react-hook-form';
import { Text, View } from 'react-native';
import { useOnboardingContext } from '~/components/context/OnboardingContext';
import { Icon } from '~/components/icons/Icon';
import { Button } from '~/components/primitive/Button';
import { FeatureUnavailableAlert } from '~/components/primitive/FeatureUnavailableAlert';
import { Input } from '~/components/primitive/Input';
import { tw } from '~/lib/tailwind';
import { OnboardingStackScreenProps } from '~/navigation/OnboardingNavigator';

import { OnboardingContainer, OnboardingDescription, OnboardingTitle } from './GetStarted';

const NewLibraryScreen = ({ navigation }: OnboardingStackScreenProps<'NewLibrary'>) => {
	const form = useOnboardingContext().forms.useForm('NewLibrary');

	const handleNewLibrary = form.handleSubmit(() => {
		Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light);
		navigation.navigate('Privacy');
	});

	const handleImport = () => FeatureUnavailableAlert();

	return (
		<OnboardingContainer>
			<Icon name="Database" style={tw`h-25 w-25`} />
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
				<Button style={tw`opacity-50`} onPress={handleImport} variant="outline">
					<Text style={tw`text-center font-medium text-ink`}>Import Library</Text>
				</Button>
			</View>
		</OnboardingContainer>
	);
};

export default NewLibraryScreen;
