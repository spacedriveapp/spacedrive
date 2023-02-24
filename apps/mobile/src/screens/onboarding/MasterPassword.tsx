import React from 'react';
import { Text, View } from 'react-native';
import { OnboardingStackScreenProps } from '~/navigation/OnboardingNavigator';

const MasterPasswordScreen = ({ navigation }: OnboardingStackScreenProps<'MasterPassword'>) => {
	return (
		<View>
			<Text>MasterPasswordScreen</Text>
		</View>
	);
};

export default MasterPasswordScreen;
