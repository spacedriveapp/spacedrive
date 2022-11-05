import React from 'react';
import { Text, View } from 'react-native';
import { SettingsStackScreenProps } from '~/navigation/SettingsNavigator';

const SupportScreen = ({ navigation }: SettingsStackScreenProps<'Support'>) => {
	return (
		<View>
			<Text>SupportScreen</Text>
		</View>
	);
};

export default SupportScreen;
