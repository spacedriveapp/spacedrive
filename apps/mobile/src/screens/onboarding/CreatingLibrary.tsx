import React from 'react';
import { Text } from 'react-native';
import { OnboardingStackScreenProps } from '~/navigation/OnboardingNavigator';
import { OnboardingContainer } from './GetStarted';

const CreatingLibraryScreen = ({ navigation }: OnboardingStackScreenProps<'CreatingLibrary'>) => {
	return (
		<OnboardingContainer>
			<Text>CreatingLibraryScreen</Text>
		</OnboardingContainer>
	);
};

export default CreatingLibraryScreen;
