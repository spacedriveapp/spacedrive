import { DrawerNavigationHelpers } from '@react-navigation/drawer/lib/typescript/src/types';
import { useNavigation } from '@react-navigation/native';
import { List } from 'phosphor-react-native';
import React from 'react';
import { Alert, Pressable, Text, View } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

import tw from '../../lib/tailwind';

const Header = () => {
	const navigation = useNavigation<DrawerNavigationHelpers>();

	const { top } = useSafeAreaInsets();

	return (
		<View style={tw.style('mx-4 bg-gray-550 rounded-md', { marginTop: top + 20 })}>
			<View style={tw`flex flex-row items-center h-11`}>
				<Pressable style={tw`px-3 h-full justify-center`} onPress={() => navigation.openDrawer()}>
					<List size={20} color={'white'} />
				</Pressable>
				<Pressable style={tw`flex-1 h-full justify-center`} onPress={() => Alert.alert('TODO')}>
					<Text style={tw`text-gray-300 font-semibold`}>Search</Text>
				</Pressable>
			</View>
		</View>
	);
};

export default Header;
