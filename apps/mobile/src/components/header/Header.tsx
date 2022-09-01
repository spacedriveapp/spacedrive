import tw from '@app/lib/tailwind';
import { useDrawerStatus } from '@react-navigation/drawer';
import { DrawerNavigationHelpers } from '@react-navigation/drawer/lib/typescript/src/types';
import { useNavigation } from '@react-navigation/native';
import { MotiView } from 'moti';
import { List } from 'phosphor-react-native';
import React from 'react';
import { Pressable, Text, View } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

const Header = () => {
	const navigation = useNavigation<DrawerNavigationHelpers>();

	const { top } = useSafeAreaInsets();

	const isDrawerOpen = useDrawerStatus() === 'open';

	return (
		<View
			style={tw.style('mx-4 bg-gray-500 border-[#333949] bg-opacity-40 rounded-md', {
				marginTop: top + 10
			})}
		>
			<View style={tw`flex flex-row items-center h-10`}>
				<Pressable style={tw`px-3 h-full justify-center`} onPress={() => navigation.openDrawer()}>
					<MotiView
						animate={{ rotate: isDrawerOpen ? '90deg' : '0deg' }}
						transition={{ type: 'timing' }}
					>
						<List size={20} color={tw.color('gray-300')} weight="fill" />
					</MotiView>
				</Pressable>
				<Pressable
					style={tw`flex-1 h-full justify-center`}
					onPress={() => navigation.navigate('Search')}
				>
					<Text style={tw`text-gray-300 font-medium text-sm`}>Search</Text>
				</Pressable>
			</View>
		</View>
	);
};

export default Header;
