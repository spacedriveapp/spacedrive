import React from 'react';
import { Text, View } from 'react-native';
import { OnboardingStackScreenProps } from '~/navigation/OnboardingNavigator';

const PrivacyScreen = ({ navigation }: OnboardingStackScreenProps<'Privacy'>) => {
	return (
		<View>
			<Text>PrivacyScreen</Text>
		</View>
	);
};

export default PrivacyScreen;
