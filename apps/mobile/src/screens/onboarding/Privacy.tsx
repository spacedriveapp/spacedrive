import React from 'react';
import { Text, View } from 'react-native';
import { OnboardingStackScreenProps } from '~/navigation/OnboardingNavigator';
import { OnboardingContainer } from './GetStarted';

const PrivacyScreen = ({ navigation }: OnboardingStackScreenProps<'Privacy'>) => {
	return (
		<OnboardingContainer>
			<Text>PrivacyScreen</Text>
		</OnboardingContainer>
	);
};

export default PrivacyScreen;
