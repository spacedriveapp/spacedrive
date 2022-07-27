import React from 'react';
import { Text, View } from 'react-native';

import DrawerScreenWrapper from '../../components/drawer/DrawerScreenWrapper';
import tw from '../../lib/tailwind';
import { HomeDrawerScreenProps } from '../../types/navigation';

export default function SettingsScreen({ navigation }: HomeDrawerScreenProps<'Settings'>) {
	return (
		<DrawerScreenWrapper>
			<Text style={tw`font-bold text-xl text-white`}>Settings</Text>
			<View style={tw`my-8 h-1 w-4/5`} />
		</DrawerScreenWrapper>
	);
}
