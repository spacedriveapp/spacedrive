import React from 'react';
import { Text, View } from 'react-native';
import { SettingsStackScreenProps } from '~/navigation/SettingsNavigator';

const PrivacySettingsScreen = ({ navigation }: SettingsStackScreenProps<'PrivacySettings'>) => {
	return (
		<View>
			<Text>PrivacySettingsScreen</Text>
		</View>
	);
};

export default PrivacySettingsScreen;
