import { Alert, Image } from 'react-native';
import { getOnboardingStore, useOnboardingStore } from '@sd/client';
import { useZodForm, z } from '~/hooks/useZodForm';
import { tw } from '~/lib/tailwind';
import { OnboardingStackScreenProps } from '~/navigation/OnboardingNavigator';
import { OnboardingContainer, OnboardingDescription, OnboardingTitle } from './GetStarted';

const schema = z.object({
	name: z.string()
});

const NewLibraryScreen = ({ navigation }: OnboardingStackScreenProps<'NewLibrary'>) => {
	const obStore = useOnboardingStore();

	const form = useZodForm({
		schema,
		defaultValues: {
			name: obStore.newLibraryName
		}
	});

	const onSubmit = form.handleSubmit(async (data) => {
		getOnboardingStore().newLibraryName = data.name;
		navigation.navigate('MasterPassword');
	});

	const handleImport = () => {
		Alert.alert('TODO');
	};

	return (
		<OnboardingContainer>
			<Image source={require('@sd/assets/images/Database.png')} style={tw`h-25 w-25`} />
			<OnboardingTitle>Create a Library</OnboardingTitle>
			<OnboardingDescription>
				Libraries are a secure, on-device database. Your files remain where they are, the Library
				catalogs them and stores all Spacedrive related data.
			</OnboardingDescription>
		</OnboardingContainer>
	);
};

export default NewLibraryScreen;
