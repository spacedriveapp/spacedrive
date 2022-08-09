import React from 'react';
import { Text } from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';

import DrawerScreenWrapper from '../components/drawer/DrawerScreenWrapper';
import tw from '../lib/tailwind';
import { SpacesStackScreenProps } from '../navigation/tabs/SpacesStack';

export default function SpacesScreen({ navigation }: SpacesStackScreenProps<'Spaces'>) {
	return (
		<DrawerScreenWrapper>
			<Text style={tw`font-bold text-xl text-white`}>Spaces</Text>
		</DrawerScreenWrapper>
	);
}
