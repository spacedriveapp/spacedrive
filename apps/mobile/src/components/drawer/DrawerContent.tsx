import { DrawerContentComponentProps, DrawerContentScrollView } from '@react-navigation/drawer';
import { CirclesFour, Planet } from 'phosphor-react-native';
import React from 'react';
import { ColorValue, Pressable, Text, View } from 'react-native';
import { PhotographIcon } from 'react-native-heroicons/outline';
import { CogIcon } from 'react-native-heroicons/solid';

import Layout from '../../constants/Layout';
import tw from '../../lib/tailwind';
import DrawerItem from './DrawerItem';
import DrawerLocationItem from './DrawerLocationItem';
import DrawerTagItem from './DrawerTagItem';

const drawerHeight = Layout.window.height * 0.85;

const Heading: React.FC<{ text: string }> = ({ text }) => (
	<Text style={tw`mt-5 mb-2 ml-1 text-xs font-semibold text-gray-300`}>{text}</Text>
);

const DrawerContent = ({ descriptors, navigation, state }: DrawerContentComponentProps) => {
	// console.log(state.index);
	return (
		<DrawerContentScrollView style={tw`flex-1 p-4`} scrollEnabled={false}>
			<View style={tw.style('justify-between', { height: drawerHeight })}>
				<View>
					<Text style={tw`my-4 text-white`}>TODO: Library Selection</Text>
					<DrawerItem
						label={'Overview'}
						icon={<Planet size={20} color={'white'} weight="bold" />}
						onPress={() => navigation.navigate('Overview')}
						isSelected={state.index === 0}
					/>
					<DrawerItem
						label={'Spaces'}
						onPress={() => navigation.navigate('Spaces')}
						icon={<CirclesFour size={20} color={'white'} weight="bold" />}
						isSelected={state.index === 1}
					/>
					<DrawerItem
						label={'Photos'}
						onPress={() => navigation.navigate('Photos')}
						icon={<PhotographIcon size={20} color={'white'} />}
						isSelected={state.index === 2}
					/>
					{/* Locations */}
					<Heading text="Locations" />
					<DrawerLocationItem
						folderName="Spacedrive"
						// Both fields under this is temporary, we will have several locations and we will navigate with id or smth.
						onPress={() => navigation.navigate('Location')}
						isSelected={state.index === 3}
					/>
					<DrawerLocationItem
						folderName="Classified"
						// Both fields under this is temporary, we will have several locations and we will navigate with id or smth.
						onPress={() => navigation.navigate('Location')}
						isSelected={state.index === 99}
					/>
					{/* Add Location */}
					<View style={tw`border border-dashed rounded border-gray-450 border-opacity-60 mt-1`}>
						<Text style={tw`text-xs font-bold text-center text-gray-400 px-2 py-2`}>
							Add Location
						</Text>
					</View>
					{/* Tags */}
					<Heading text="Tags" />
					<DrawerTagItem
						tagName="Secret"
						isSelected={state.index === 5}
						onPress={() => navigation.navigate('Tag')}
						tagColor={tw.color('purple-500') as ColorValue}
					/>
					<DrawerTagItem
						tagName="OBS"
						isSelected={state.index === 5}
						onPress={() => navigation.navigate('Tag')}
						tagColor={tw.color('blue-500') as ColorValue}
					/>
					<DrawerTagItem
						tagName="BlackMagic"
						isSelected={state.index === 5}
						onPress={() => navigation.navigate('Tag')}
						tagColor={tw.color('red-500') as ColorValue}
					/>
				</View>
				<Pressable onPress={() => navigation.navigate('Settings')}>
					<CogIcon color="white" size={24} />
				</Pressable>
			</View>
		</DrawerContentScrollView>
	);
};

export default DrawerContent;
