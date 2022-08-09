import React from 'react';
import { Text, View } from 'react-native';

import DrawerScreenWrapper from '../components/drawer/DrawerScreenWrapper';
import tw from '../lib/tailwind';
import { SharedScreenProps } from '../navigation/SharedScreens';

export default function LocationScreen({ navigation, route }: SharedScreenProps<'Location'>) {
	const { id } = route.params;
	return (
		<DrawerScreenWrapper>
			<Text style={tw`font-bold text-xl text-white`}>Location {id}</Text>
		</DrawerScreenWrapper>
	);
}
