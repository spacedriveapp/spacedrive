import React from 'react';
import { Text, View } from 'react-native';

import tw from '../lib/tailwind';

export default function TabTwoScreen() {
	return (
		<View style={tw`flex-1 items-center justify-center`}>
			<Text style={tw`font-bold text-xl`}>Tab Two</Text>
			<View style={tw`my-8 h-1 w-4/5`} />
		</View>
	);
}
