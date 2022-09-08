import React from 'react';
import { Text, View } from 'react-native';
import tw from '~/lib/tailwind';
import { RootStackScreenProps } from '~/navigation';

export default function SettingsScreen({ navigation }: RootStackScreenProps<'Settings'>) {
	return (
		<View style={tw`flex-1 items-center justify-center`}>
			<Text style={tw`font-bold text-xl text-white`}>Settings</Text>
			<View style={tw`my-8 h-1 w-4/5`} />
		</View>
	);
}
