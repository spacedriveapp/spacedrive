import { Text } from 'react-native';
import { useOnboardingStore } from '@sd/client';
import { OnboardingStackScreenProps } from '~/navigation/OnboardingNavigator';
import { OnboardingContainer } from './GetStarted';

const NewLibraryScreen = ({ navigation }: OnboardingStackScreenProps<'NewLibrary'>) => {
	const obStore = useOnboardingStore();
	return (
		<OnboardingContainer>
			<Text>New Library</Text>
		</OnboardingContainer>
	);
};

export default NewLibraryScreen;
