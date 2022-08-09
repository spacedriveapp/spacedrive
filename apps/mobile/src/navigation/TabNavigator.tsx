import { BottomTabScreenProps, createBottomTabNavigator } from '@react-navigation/bottom-tabs';
import { CompositeScreenProps } from '@react-navigation/native';
import { CirclesFour, Folder, Planet } from 'phosphor-react-native';
import { PhotographIcon } from 'react-native-heroicons/outline';

import tw from '../lib/tailwind';
import OverviewScreen from '../screens/Overview';
import PhotosScreen from '../screens/Photos';
import SpacesScreen from '../screens/Spaces';
import type { HomeDrawerScreenProps } from './DrawerNavigator';
import BrowseStack from './tabs/BrowseStack';

const Tab = createBottomTabNavigator<TabParamList>();

export default function TabNavigator() {
	return (
		<Tab.Navigator
			initialRouteName="Overview"
			screenOptions={{
				headerShown: false,
				tabBarActiveTintColor: tw.color('bg-primary'),
				tabBarInactiveTintColor: 'white',
				tabBarStyle: {
					backgroundColor: '#08090D',
					borderTopColor: 'transparent'
				}
			}}
		>
			<Tab.Screen
				name="Overview"
				component={OverviewScreen}
				options={{
					tabBarIcon: ({ focused }) => (
						<Planet size={22} weight="bold" color={focused ? tw.color('bg-primary') : 'white'} />
					)
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
				name="Spaces"
				component={SpacesScreen}
				options={{
					tabBarIcon: ({ focused }) => (
						<CirclesFour
							size={22}
							weight="bold"
							color={focused ? tw.color('bg-primary') : 'white'}
						/>
					)
				}}
			/>
			<Tab.Screen
				name="Photos"
				component={PhotosScreen}
				options={{
					tabBarIcon: ({ focused }) => (
						<PhotographIcon size={22} color={focused ? tw.color('bg-primary') : 'white'} />
					)
				}}
			/>
		</Tab.Navigator>
	);
}

export type TabParamList = {
	Overview: undefined;
	BrowseStack: undefined;
	Spaces: undefined;
	Photos: undefined;
};

export type TabScreenProps<Screen extends keyof TabParamList> = CompositeScreenProps<
	BottomTabScreenProps<TabParamList, Screen>,
	HomeDrawerScreenProps<'Home'>
>;
