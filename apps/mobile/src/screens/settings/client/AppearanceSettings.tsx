import React from 'react';
import { Text, View } from 'react-native';
import { SettingsStackScreenProps } from '~/navigation/SettingsNavigator';

const AppearanceSettingsScreen = ({
	navigation
}: SettingsStackScreenProps<'AppearanceSettings'>) => {
	return (
		<View>
			<Text>AppearanceSettingsScreen</Text>
		</View>
	);
};

export default AppearanceSettingsScreen;
