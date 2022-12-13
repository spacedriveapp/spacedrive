import React from 'react';
import { Text, View } from 'react-native';
import tw from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/SettingsNavigator';

const AboutScreen = ({ navigation }: SettingsStackScreenProps<'About'>) => {
	return (
		<View>
			<Text style={tw`text-ink`}>TODO</Text>
		</View>
	);
};

export default AboutScreen;
