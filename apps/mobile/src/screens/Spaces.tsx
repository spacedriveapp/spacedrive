import React from 'react';
import { Text, View } from 'react-native';

import { Button } from '../components/base/Button';
import DrawerScreenWrapper from '../components/drawer/DrawerScreenWrapper';
import tw from '../lib/tailwind';
import { BottomNavScreenProps } from '../types/navigation';

export default function SpacesScreen({ navigation }: BottomNavScreenProps<'Spaces'>) {
	return (
		<DrawerScreenWrapper>
			<Text style={tw`font-bold text-xl text-white`}>Spaces</Text>
			<View style={tw`my-8 h-1 w-4/5`} />
			<Button variant="primary" size="lg" onPress={() => navigation.openDrawer()}>
				<Text style={tw`font-bold text-white`}>Open Drawer</Text>
			</Button>
		</DrawerScreenWrapper>
	);
}
