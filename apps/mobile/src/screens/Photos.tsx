import React from 'react';
import { Text, View } from 'react-native';

import tw from '../lib/tailwind';
import type { TabScreenProps } from '../navigation/TabNavigator';

export default function PhotosScreen({ navigation }: TabScreenProps<'Photos'>) {
	return (
		<View style={tw`flex-1 items-center justify-center`}>
			<Text style={tw`font-bold text-xl text-white`}>Photos</Text>
		</View>
	);
}
