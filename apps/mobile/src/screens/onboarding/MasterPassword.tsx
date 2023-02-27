import React from 'react';
import { Text, View } from 'react-native';
import { OnboardingStackScreenProps } from '~/navigation/OnboardingNavigator';
import { OnboardingContainer } from './GetStarted';

const MasterPasswordScreen = ({ navigation }: OnboardingStackScreenProps<'MasterPassword'>) => {
	return (
		<OnboardingContainer>
			<Text>MasterPasswordScreen</Text>
		</OnboardingContainer>
	);
};

export default MasterPasswordScreen;
