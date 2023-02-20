import React from 'react';
import { Text, View } from 'react-native';
import { tw } from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/SettingsNavigator';

const AppearanceSettingsScreen = ({
	navigation
}: SettingsStackScreenProps<'AppearanceSettings'>) => {
	return (
		<View>
			<Text style={tw`text-ink`}>TODO: Theme Switch</Text>
		</View>
	);
};

export default AppearanceSettingsScreen;
