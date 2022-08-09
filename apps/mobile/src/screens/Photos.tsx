import React from 'react';
import { Text, View } from 'react-native';

import DrawerScreenWrapper from '../components/drawer/DrawerScreenWrapper';
import tw from '../lib/tailwind';
import { PhotosStackScreenProps } from '../navigation/tabs/PhotosStack';

export default function PhotosScreen({ navigation }: PhotosStackScreenProps<'Photos'>) {
	return (
		<DrawerScreenWrapper>
			<Text style={tw`font-bold text-xl text-white`}>Photos</Text>
		</DrawerScreenWrapper>
	);
}
