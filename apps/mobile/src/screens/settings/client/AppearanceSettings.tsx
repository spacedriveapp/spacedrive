import React from 'react';
import { ScrollView, Text, View } from 'react-native';
import { Divider } from '~/components/primitive/Divider';
import { SettingsTitle } from '~/components/settings/SettingsContainer';
import { tw } from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/SettingsNavigator';

const AppearanceSettingsScreen = ({
	navigation
}: SettingsStackScreenProps<'AppearanceSettings'>) => {
	return (
		<View style={tw`flex-1 pt-4`}>
			<SettingsTitle>Theme</SettingsTitle>
		</View>
	);
};

export default AppearanceSettingsScreen;
