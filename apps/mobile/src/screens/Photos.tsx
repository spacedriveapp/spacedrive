import React from 'react';
import { Text, View } from 'react-native';

import DrawerScreenWrapper from '../components/drawer/DrawerScreenWrapper';
import tw from '../lib/tailwind';
import { BottomNavScreenProps } from '../types/navigation';

export default function PhotosScreen({ navigation }: BottomNavScreenProps<'Photos'>) {
	return (
		<DrawerScreenWrapper>
			<Text style={tw`font-bold text-xl text-white`}>Photos</Text>
			<View style={tw`my-8 h-1 w-4/5`} />
		</DrawerScreenWrapper>
	);
}
