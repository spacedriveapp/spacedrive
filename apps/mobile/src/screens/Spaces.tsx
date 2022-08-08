import React from 'react';
import { Text } from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';

import tw from '../lib/tailwind';
import type { TabScreenProps } from '../navigation/TabNavigator';

export default function SpacesScreen({ navigation }: TabScreenProps<'Spaces'>) {
	return (
		<SafeAreaView style={tw`flex-1 items-center justify-center`}>
			<Text style={tw`font-bold text-xl text-white`}>Spaces</Text>
		</SafeAreaView>
	);
}
