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
import { CirclesFour, Planet } from 'phosphor-react-native';
import React from 'react';
import { ColorValue, Pressable, Text, View } from 'react-native';
import { PhotographIcon } from 'react-native-heroicons/outline';
import { CogIcon } from 'react-native-heroicons/solid';

import Layout from '../../constants/Layout';
import tw from '../../lib/tailwind';
import { valueof } from '../../types/helper';
import { HomeDrawerParamList } from '../../types/navigation';
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

const Heading: React.FC<{ text: string }> = ({ text }) => (
	<Text style={tw`mt-5 mb-2 ml-1 text-xs font-semibold text-gray-300`}>{text}</Text>
);

// This is a hacky way to get the active route name and params but it works and it's typed...

type ActiveRoute = {
	key: string;
	name: keyof HomeDrawerParamList;
	params: valueof<HomeDrawerParamList>;
};

const getActiveRouteState = function (state: any): ActiveRoute {
	if (!state.routes || state.routes.length === 0 || state.index >= state.routes.length) {
		return state;
	}

	const childActiveRoute = state.routes[state.index];
	return getActiveRouteState(childActiveRoute);
};

// Overriding the default to add typing for our params.
type DrawerContentComponentProps = {
	state: DrawerNavigationState<HomeDrawerParamList>;
	navigation: NavigationHelpers<HomeDrawerParamList, DrawerNavigationEventMap> &
		DrawerActionHelpers<HomeDrawerParamList>;
	// descriptors type is generic
	descriptors: DrawerDescriptorMap;
};

const DrawerContent = ({ descriptors, navigation, state }: DrawerContentComponentProps) => {
	return (
		<DrawerContentScrollView style={tw`flex-1 p-4`} scrollEnabled={false}>
			<View style={tw.style('justify-between', { height: drawerHeight })}>
				<View>
					<Text style={tw`my-4 text-white`}>TODO: Library Selection</Text>
					<DrawerItem
						label={'Overview'}
						icon={<Planet size={20} color={'white'} weight="bold" />}
						onPress={() => navigation.jumpTo('Overview')}
						isSelected={getActiveRouteState(state).name === 'Overview'}
					/>
					<DrawerItem
						label={'Spaces'}
						onPress={() => navigation.jumpTo('Spaces')}
						icon={<CirclesFour size={20} color={'white'} weight="bold" />}
						isSelected={getActiveRouteState(state).name === 'Spaces'}
					/>
					<DrawerItem
						label={'Photos'}
						onPress={() => navigation.jumpTo('Photos')}
						icon={<PhotographIcon size={20} color={'white'} />}
						isSelected={getActiveRouteState(state).name === 'Photos'}
					/>
					{/* Locations */}
					<Heading text="Locations" />
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
					{/* Tags */}
					<Heading text="Tags" />
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
