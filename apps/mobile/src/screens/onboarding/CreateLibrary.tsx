import React from 'react';
import { Text, View } from 'react-native';
import { AnimatedButton } from '~/components/primitive/Button';
import { setItemToStorage } from '~/lib/storage';
import tw from '~/lib/tailwind';
import { OnboardingStackScreenProps } from '~/navigation/OnboardingNavigator';
import { useOnboardingStore } from '~/stores/useOnboardingStore';

const CreateLibraryScreen = ({ navigation }: OnboardingStackScreenProps<'CreateLibrary'>) => {
	const { hideOnboarding } = useOnboardingStore();

	function onButtonPress() {
		setItemToStorage('@onboarding', '1');
		// TODO: Add a loading indicator to button as this takes a second or so.
		hideOnboarding();
	}
	return (
		<View style={tw`flex-1 items-center justify-center bg-gray-650 p-4`}>
			<Text style={tw`text-gray-450 text-center px-6 my-8 text-base leading-relaxed`}>
				Onboarding screen for users to create their first library
			</Text>
			<AnimatedButton variant="primary" onPress={onButtonPress}>
				<Text style={tw`text-white text-center px-6 py-2 text-base font-medium`}>
					Create Library
				</Text>
			</AnimatedButton>
		</View>
	);
};

export default CreateLibraryScreen;
