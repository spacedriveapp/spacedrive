import React from 'react';
import { Text, View } from 'react-native';
import { SettingsStackScreenProps } from '~/navigation/SettingsNavigator';

const KeysSettingsScreen = ({ navigation }: SettingsStackScreenProps<'KeysSettings'>) => {
	return (
		<View>
			<Text>KeysSettingsScreen</Text>
		</View>
	);
};

export default KeysSettingsScreen;
