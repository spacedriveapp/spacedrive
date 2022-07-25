import React from 'react';
import { Text, View } from 'react-native';

import tw from '../lib/tailwind';
import { HomeDrawerScreenProps } from '../types/navigation';

export default function LocationScreen({ navigation }: HomeDrawerScreenProps<'Location'>) {
	return (
		<View style={tw`flex-1 items-center justify-center`}>
			<Text style={tw`font-bold text-xl`}>Locations</Text>
			<View style={tw`my-8 h-1 w-4/5`} />
			<Text>will receive params to show location specific data</Text>
		</View>
	);
}
