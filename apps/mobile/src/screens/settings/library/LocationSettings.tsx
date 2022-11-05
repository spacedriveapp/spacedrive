import React from 'react';
import { Text, View } from 'react-native';
import { SettingsStackScreenProps } from '~/navigation/SettingsNavigator';

const LocationSettingsScreen = ({ navigation }: SettingsStackScreenProps<'LocationSettings'>) => {
	return (
		<View>
			<Text>LocationSettingsScreen</Text>
		</View>
	);
};

export default LocationSettingsScreen;
