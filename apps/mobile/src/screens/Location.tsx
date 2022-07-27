import React from 'react';
import { Text, View } from 'react-native';

import DrawerScreenWrapper from '../components/drawer/DrawerScreenWrapper';
import tw from '../lib/tailwind';
import { HomeDrawerScreenProps } from '../types/navigation';

export default function LocationScreen({ navigation, route }: HomeDrawerScreenProps<'Location'>) {
	const { id } = route.params;
	return (
		<DrawerScreenWrapper>
			<Text style={tw`font-bold text-xl text-white`}>Locations</Text>
			<View style={tw`my-8 h-1 w-4/5`} />
			<Text style={tw`text-white`}>
				{id} --- will receive params to show location specific data
			</Text>
		</DrawerScreenWrapper>
	);
}
