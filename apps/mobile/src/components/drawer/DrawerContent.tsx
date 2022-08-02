import { DrawerContentScrollView } from '@react-navigation/drawer';
import {
	DrawerDescriptorMap,
	DrawerNavigationEventMap
} from '@react-navigation/drawer/lib/typescript/src/types';
import {
	DrawerActionHelpers,
	DrawerNavigationState,
	NavigationHelpers
} from '@react-navigation/native';
import { House } from 'phosphor-react-native';
import React from 'react';
import { ColorValue, Pressable, Text, View } from 'react-native';
import { CogIcon } from 'react-native-heroicons/solid';

import Layout from '../../constants/Layout';
import tw from '../../lib/tailwind';
import { valueof } from '../../types/helper';
import { DrawerNavParamList } from '../../types/navigation';
import CollapsibleView from '../layout/CollapsibleView';
import DrawerItem from './DrawerItem';
import DrawerLocationItem from './DrawerLocationItem';
import DrawerTagItem from './DrawerTagItem';

const placeholderLocationData = [
	{
		id: 1,
		name: 'Spacedrive'
	},
	{
		id: 2,
		name: 'Classified'
	}
];
const placeholderTagsData = [
	{
		id: 1,
		name: 'Secret',
		color: tw.color('blue-500')
	},
	{
		id: 2,
		name: 'OBS',
		color: tw.color('purple-500')
	},
	{
		id: 3,
		name: 'BlackMagic',
		color: tw.color('red-500')
	}
];

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
interface DrawerContentComponentProps {
	state: DrawerNavigationState<DrawerNavParamList>;
	navigation: NavigationHelpers<DrawerNavParamList, DrawerNavigationEventMap> &
		DrawerActionHelpers<DrawerNavParamList>;
	// descriptors type is generic
	descriptors: DrawerDescriptorMap;
}

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
					{/* Locations */}
					<CollapsibleView
						title="Locations"
						titleStyle={tw`mt-5 mb-3 ml-1 text-sm font-semibold text-gray-300`}
					>
						{placeholderLocationData.map((location) => (
							<DrawerLocationItem
								key={location.id}
								folderName={location.name}
								onPress={() => navigation.jumpTo('Location', { id: location.id })}
								isSelected={
									getActiveRouteState(state).name === 'Location' &&
									getActiveRouteState(state).params?.id === location.id
								}
							/>
						))}
						{/* Add Location */}
						<View style={tw`border border-dashed rounded border-gray-450 border-opacity-60 mt-1`}>
							<Text style={tw`text-xs font-bold text-center text-gray-400 px-2 py-2`}>
								Add Location
							</Text>
						</View>
					</CollapsibleView>
					{/* Tags */}
					<CollapsibleView
						title="Tags"
						titleStyle={tw`mt-5 mb-3 ml-1 text-sm font-semibold text-gray-300`}
					>
						{placeholderTagsData.map((tag) => (
							<DrawerTagItem
								key={tag.id}
								tagName={tag.name}
								onPress={() => navigation.jumpTo('Tag', { id: tag.id })}
								tagColor={tag.color as ColorValue}
								isSelected={
									getActiveRouteState(state).name === 'Tag' &&
									getActiveRouteState(state).params?.id === tag.id
								}
							/>
						))}
					</CollapsibleView>
				</View>
				{/* Settings */}
				<Pressable onPress={() => navigation.jumpTo('Settings')}>
					<CogIcon color="white" size={24} />
				</Pressable>
			</View>
		</DrawerContentScrollView>
	);
};

export default DrawerContent;
