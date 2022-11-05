import React from 'react';
import { Text, View } from 'react-native';
import { SettingsStackScreenProps } from '~/navigation/SettingsNavigator';

const LibraryGeneralSettingsScreen = ({
	navigation
}: SettingsStackScreenProps<'LibraryGeneralSettings'>) => {
	return (
		<View>
			<Text>LibraryGeneralSettingsScreen</Text>
		</View>
	);
};

export default LibraryGeneralSettingsScreen;
