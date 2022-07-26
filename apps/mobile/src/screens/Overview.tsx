import React from 'react';
import { Text, View } from 'react-native';

import { Button } from '../components/base/Button';
import tw from '../lib/tailwind';
import { HomeDrawerScreenProps } from '../types/navigation';

export default function OverviewScreen({ navigation }: HomeDrawerScreenProps<'Overview'>) {
	return (
		<View style={tw`flex-1 items-center justify-center bg-[#121219]`}>
			<Text style={tw`font-bold text-xl text-white`}>Overview</Text>
			<View style={tw`my-8 h-1 w-4/5`} />
			<Button variant="primary" size="lg" onPress={() => navigation.openDrawer()}>
				<Text style={tw`font-bold text-white`}>Open Drawer</Text>
			</Button>
		</View>
	);
}
