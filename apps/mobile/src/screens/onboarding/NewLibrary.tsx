import { Text, View } from 'react-native';
import CreateLibraryDialog from '~/components/dialog/CreateLibraryDialog';
import { AnimatedButton } from '~/components/primitive/Button';
import { tw } from '~/lib/tailwind';
import { OnboardingStackScreenProps } from '~/navigation/OnboardingNavigator';

const NewLibraryScreen = ({ navigation }: OnboardingStackScreenProps<'NewLibrary'>) => {
	return (
		<View style={tw`bg-app flex-1 items-center justify-center p-4`}>
			<Text style={tw`text-ink-dull my-8 px-6 text-center text-base leading-relaxed`}>
				Onboarding screen for users to create their first library
			</Text>
			<CreateLibraryDialog disableBackdropClose>
				<AnimatedButton variant="accent">
					<Text style={tw`text-ink px-6 py-2 text-center text-base font-medium`}>
						Create Library
					</Text>
				</AnimatedButton>
			</CreateLibraryDialog>
		</View>
	);
};

export default NewLibraryScreen;
