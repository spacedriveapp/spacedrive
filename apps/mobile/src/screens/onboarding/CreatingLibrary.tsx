import React from 'react';
import { Text, View } from 'react-native';
import { OnboardingStackScreenProps } from '~/navigation/OnboardingNavigator';

const CreatingLibraryScreen = ({ navigation }: OnboardingStackScreenProps<'CreatingLibrary'>) => {
	return (
		<View>
			<Text>CreatingLibraryScreen</Text>
		</View>
	);
};

export default CreatingLibraryScreen;
