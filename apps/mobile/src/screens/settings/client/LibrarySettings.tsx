import React from 'react';
import { Text, View } from 'react-native';
import { SettingsStackScreenProps } from '~/navigation/SettingsNavigator';

const LibrarySettingsScreen = ({ navigation }: SettingsStackScreenProps<'LibrarySettings'>) => {
	return (
		<View>
			<Text>LibrarySettingsScreen</Text>
		</View>
	);
};

export default LibrarySettingsScreen;
