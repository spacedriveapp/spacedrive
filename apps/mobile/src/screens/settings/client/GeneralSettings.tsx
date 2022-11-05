import React from 'react';
import { Text, View } from 'react-native';
import { SettingsStackScreenProps } from '~/navigation/SettingsNavigator';

const GeneralSettingsScreen = ({ navigation }: SettingsStackScreenProps<'GeneralSettings'>) => {
	return (
		<View>
			<Text>GeneralSettingsScreen</Text>
		</View>
	);
};

export default GeneralSettingsScreen;
