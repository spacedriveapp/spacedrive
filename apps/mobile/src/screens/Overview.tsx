import React from 'react';
import { Text, View } from 'react-native';

import { Button } from '../components/base/Button';
import DrawerScreenWrapper from '../components/drawer/DrawerScreenWrapper';
import tw from '../lib/tailwind';
import { BottomNavScreenProps } from '../types/navigation';

export default function OverviewScreen({ navigation }: BottomNavScreenProps<'Overview'>) {
	return (
		<DrawerScreenWrapper>
			<Text style={tw`font-bold text-xl text-white`}>Overview</Text>
			<View style={tw`my-8 h-1 w-4/5`} />

			<Button variant="primary" size="lg" onPress={() => navigation.openDrawer()}>
				<Text style={tw`font-bold text-white`}>Open Drawer</Text>
			</Button>
			<View style={tw`my-8 h-1 w-4/5`} />
			<Button variant="primary" size="lg" onPress={() => navigation.navigate('Modal')}>
				<Text style={tw`font-bold text-white`}>Open Modal</Text>
			</Button>
		</DrawerScreenWrapper>
	);
}
