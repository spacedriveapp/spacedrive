import React from 'react';
import { Text, View } from 'react-native';
import { SettingsStackScreenProps } from '~/navigation/SettingsNavigator';

const ExtensionsSettingsScreen = ({
	navigation
}: SettingsStackScreenProps<'ExtensionsSettings'>) => {
	return (
		<View>
			<Text>ExtensionsSettingsScreen</Text>
		</View>
	);
};

export default ExtensionsSettingsScreen;
