import React from 'react';
import { Text, View } from 'react-native';
import { SettingsStackScreenProps } from '~/navigation/SettingsNavigator';

const AboutScreen = ({ navigation }: SettingsStackScreenProps<'About'>) => {
	return (
		<View>
			<Text>AboutScreen</Text>
		</View>
	);
};

export default AboutScreen;
