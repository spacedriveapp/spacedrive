import React from 'react';
import { Text, View } from 'react-native';

import tw from '../lib/tailwind';
import { HomeDrawerScreenProps } from '../types/navigation';

export default function TagScreen({ navigation, route }: HomeDrawerScreenProps<'Tag'>) {
	const { id } = route.params;
	return (
		<View style={tw`flex-1 items-center justify-center`}>
			<Text style={tw`font-bold text-xl`}>Tags</Text>
			<View style={tw`my-8 h-1 w-4/5`} />
			<Text>{id} --- will receive tag id to show specific data.</Text>
		</View>
	);
}
