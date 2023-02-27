import { Text } from 'react-native';
import CreateLibraryDialog from '~/components/dialog/CreateLibraryDialog';
import { AnimatedButton } from '~/components/primitive/Button';
import { tw } from '~/lib/tailwind';
import { OnboardingStackScreenProps } from '~/navigation/OnboardingNavigator';
import { OnboardingContainer } from './GetStarted';

const NewLibraryScreen = ({ navigation }: OnboardingStackScreenProps<'NewLibrary'>) => {
	return (
		<OnboardingContainer>
			<Text>New Library</Text>
			<CreateLibraryDialog disableBackdropClose>
				<AnimatedButton variant="accent">
					<Text style={tw`text-ink px-6 py-2 text-center text-base font-medium`}>
						Create Library
					</Text>
				</AnimatedButton>
			</CreateLibraryDialog>
		</OnboardingContainer>
	);
};

export default NewLibraryScreen;
