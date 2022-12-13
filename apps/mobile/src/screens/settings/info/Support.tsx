import React from 'react';
import { Text, View } from 'react-native';
import tw from '~/lib/tailwind';
import { SettingsStackScreenProps } from '~/navigation/SettingsNavigator';

const SupportScreen = ({ navigation }: SettingsStackScreenProps<'Support'>) => {
	return (
		<View>
			<Text style={tw`text-ink`}>TODO</Text>
		</View>
	);
};

export default SupportScreen;
