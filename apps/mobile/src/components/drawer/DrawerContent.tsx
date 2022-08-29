import { DrawerContentScrollView } from '@react-navigation/drawer';
import { DrawerContentComponentProps } from '@react-navigation/drawer/lib/typescript/src/types';
import { getFocusedRouteNameFromRoute } from '@react-navigation/native';
import React from 'react';
import { ColorValue, Platform, Pressable, Text, View } from 'react-native';
import { CogIcon } from 'react-native-heroicons/solid';

import Layout from '../../constants/Layout';
import tw from '../../lib/tailwind';
import CollapsibleView from '../layout/CollapsibleView';
import DrawerLocationItem from './DrawerLocationItem';
import DrawerLogo from './DrawerLogo';
import DrawerTagItem from './DrawerTagItem';

const placeholderLocationData = [
	{
		id: 1,
		name: 'Spacedrive'
	},
	{
		id: 2,
		name: 'Content'
	}
];
const placeholderTagsData = [
	{
		id: 1,
		name: 'Funny',
		color: tw.color('blue-500')
	},
	{
		id: 2,
		name: 'Twitch',
		color: tw.color('purple-500')
	},
	{
		id: 3,
		name: 'BlackMagic',
		color: tw.color('red-500')
	}
];

const drawerHeight = Platform.select({
	ios: Layout.window.height * 0.85,
	android: Layout.window.height * 0.9
});

const getActiveRouteState = function (state: any) {
	if (!state.routes || state.routes.length === 0 || state.index >= state.routes.length) {
		return state;
	}
	const childActiveRoute = state.routes[state.index];
	return getActiveRouteState(childActiveRoute);
};

const DrawerContent = ({ navigation, state }: DrawerContentComponentProps) => {
	const stackName = getFocusedRouteNameFromRoute(getActiveRouteState(state)) ?? 'OverviewStack';

	return (
		<DrawerContentScrollView style={tw`flex-1 px-4 py-2`} scrollEnabled={false}>
			<View style={tw.style('justify-between', { height: drawerHeight })}>
				<View>
					<DrawerLogo />
					<Text style={tw`my-4 text-white text-xs`}>TODO: Library Selection</Text>
					{/* Locations */}
					<CollapsibleView
						title="Locations"
						titleStyle={tw`mt-4 mb-3 ml-1 text-sm font-semibold text-gray-300`}
					>
						{placeholderLocationData.map((location) => (
							<DrawerLocationItem
								key={location.id}
								folderName={location.name}
								onPress={() =>
									navigation.navigate(stackName, {
										screen: 'Location',
										params: { id: location.id }
									})
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
						titleStyle={tw`mt-6 mb-3 ml-1 text-sm font-semibold text-gray-300`}
					>
						{placeholderTagsData.map((tag) => (
							<DrawerTagItem
								key={tag.id}
								tagName={tag.name}
								onPress={() =>
									navigation.navigate(stackName, {
										screen: 'Tag',
										params: { id: tag.id }
									})
								}
								tagColor={tag.color as ColorValue}
							/>
						))}
					</CollapsibleView>
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
