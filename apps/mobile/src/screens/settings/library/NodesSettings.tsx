import React from 'react';
import { Text, View } from 'react-native';
import { SettingsStackScreenProps } from '~/navigation/SettingsNavigator';

const NodesSettingsScreen = ({ navigation }: SettingsStackScreenProps<'NodesSettings'>) => {
	return (
		<View>
			<Text>NodesSettingsScreen</Text>
		</View>
	);
};

export default NodesSettingsScreen;
