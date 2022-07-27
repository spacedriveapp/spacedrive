import React from 'react';
import { Text, View } from 'react-native';

import DrawerScreenWrapper from '../components/drawer/DrawerScreenWrapper';
import tw from '../lib/tailwind';
import { HomeDrawerScreenProps } from '../types/navigation';

export default function TagScreen({ navigation, route }: HomeDrawerScreenProps<'Tag'>) {
	const { id } = route.params;
	return (
		<DrawerScreenWrapper>
			<Text style={tw`font-bold text-xl text-white`}>Tags</Text>
			<View style={tw`my-8 h-1 w-4/5`} />
			<Text>{id} --- will receive tag id to show specific data.</Text>
		</DrawerScreenWrapper>
	);
}
