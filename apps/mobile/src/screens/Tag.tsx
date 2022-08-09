import React from 'react';
import { Text, View } from 'react-native';

import DrawerScreenWrapper from '../components/drawer/DrawerScreenWrapper';
import tw from '../lib/tailwind';
import { SharedScreenProps } from '../navigation/SharedScreens';

export default function TagScreen({ navigation, route }: SharedScreenProps<'Tag'>) {
	const { id } = route.params;
	return (
		<DrawerScreenWrapper>
			<Text style={tw`font-bold text-xl text-white`}>Tag {id}</Text>
		</DrawerScreenWrapper>
	);
}
