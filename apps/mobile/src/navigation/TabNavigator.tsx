import { BottomTabScreenProps, createBottomTabNavigator } from '@react-navigation/bottom-tabs';
import { CompositeScreenProps, NavigatorScreenParams } from '@react-navigation/native';
import { Camera, CirclesFour, Folder, Planet } from 'phosphor-react-native';
import tw from '~/lib/tailwind';

import type { HomeDrawerScreenProps } from './DrawerNavigator';
import BrowseStack, { BrowseStackParamList } from './tabs/BrowseStack';
import OverviewStack, { OverviewStackParamList } from './tabs/OverviewStack';
import PhotosStack, { PhotosStackParamList } from './tabs/PhotosStack';
import SpacesStack, { SpacesStackParamList } from './tabs/SpacesStack';

const Tab = createBottomTabNavigator<TabParamList>();

export default function TabNavigator() {
	return (
		<Tab.Navigator
			initialRouteName="OverviewStack"
			screenOptions={{
				headerShown: false,
				tabBarActiveTintColor: tw.color('primary'),
				tabBarInactiveTintColor: 'white',
				tabBarStyle: {
					backgroundColor: tw.color('gray-650'),
					borderTopColor: tw.color('gray-600')
				}
			}}
		>
			<Tab.Screen
				name="OverviewStack"
				component={OverviewStack}
				options={{
					tabBarIcon: ({ focused }) => (
						<Planet size={22} weight="bold" color={focused ? tw.color('bg-primary') : 'white'} />
					),
					tabBarLabel: 'Overview'
				}}
			/>
			<Tab.Screen
				name="BrowseStack"
				component={BrowseStack}
				options={{
					tabBarIcon: ({ focused }) => (
						<Folder size={22} weight="bold" color={focused ? tw.color('bg-primary') : 'white'} />
					),
					tabBarLabel: 'Browse'
				}}
			/>
			<Tab.Screen
				name="SpacesStack"
				component={SpacesStack}
				options={{
					tabBarIcon: ({ focused }) => (
						<CirclesFour
							size={22}
							weight="bold"
							color={focused ? tw.color('bg-primary') : 'white'}
						/>
					),
					tabBarLabel: 'Spaces'
				}}
			/>
			<Tab.Screen
				name="PhotosStack"
				component={PhotosStack}
				options={{
					tabBarIcon: ({ focused }) => (
						<Camera size={22} weight="bold" color={focused ? tw.color('bg-primary') : 'white'} />
					),
					tabBarLabel: 'Photos'
				}}
			/>
		</Tab.Navigator>
	);
}

export type TabParamList = {
	OverviewStack: NavigatorScreenParams<OverviewStackParamList>;
	BrowseStack: NavigatorScreenParams<BrowseStackParamList>;
	SpacesStack: NavigatorScreenParams<SpacesStackParamList>;
	PhotosStack: NavigatorScreenParams<PhotosStackParamList>;
};

export type TabScreenProps<Screen extends keyof TabParamList> = CompositeScreenProps<
	BottomTabScreenProps<TabParamList, Screen>,
	HomeDrawerScreenProps<'Home'>
>;
