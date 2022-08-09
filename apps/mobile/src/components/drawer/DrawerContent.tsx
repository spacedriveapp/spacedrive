import { DrawerContentScrollView } from '@react-navigation/drawer';
import { DrawerContentComponentProps } from '@react-navigation/drawer/lib/typescript/src/types';
import { House } from 'phosphor-react-native';
import React from 'react';
import { Pressable, Text, View } from 'react-native';
import { CogIcon } from 'react-native-heroicons/solid';

import Layout from '../../constants/Layout';
import tw from '../../lib/tailwind';
import type { DrawerNavParamList } from '../../navigation/DrawerNavigator';
import { valueof } from '../../types/helper';
import DrawerItem from './DrawerItem';

const drawerHeight = Layout.window.height * 0.85;

// This is a hacky way to get the active route name and params but it works and it's typed...

interface ActiveRoute {
	key: string;
	name: keyof DrawerNavParamList;
	params: valueof<Omit<DrawerNavParamList, 'Home'>>;
}

const getActiveRouteState = function (state: any): ActiveRoute {
	if (!state.routes || state.routes.length === 0 || state.index >= state.routes.length) {
		return state;
	}

	const childActiveRoute = state.routes[state.index];
	return getActiveRouteState(childActiveRoute);
};

// Overriding the default to add typing for our params.
// interface DrawerContentComponentProps {
// 	state: DrawerNavigationState<DrawerNavParamList>;
// 	navigation: NavigationHelpers<DrawerNavParamList, DrawerNavigationEventMap> &
// 		DrawerActionHelpers<DrawerNavParamList>;
// 	// descriptors type is generic
// 	descriptors: DrawerDescriptorMap;
// }

const DrawerContent = ({ descriptors, navigation, state }: DrawerContentComponentProps) => {
	return (
		<DrawerContentScrollView style={tw`flex-1 p-4`} scrollEnabled={false}>
			<View style={tw.style('justify-between', { height: drawerHeight })}>
				<View>
					<Text style={tw`my-4 text-white`}>TODO: Library Selection</Text>
					<DrawerItem
						label={'Home'}
						icon={<House size={20} color={'white'} weight="bold" />}
						onPress={() => navigation.jumpTo('Home')}
						isSelected={getActiveRouteState(state).name === 'Home'}
					/>
				</View>
				{/* Settings */}
				<Pressable onPress={() => navigation.navigate('Settings')}>
					<CogIcon color="white" size={24} />
				</Pressable>
			</View>
		</DrawerContentScrollView>
	);
};

export default DrawerContent;
