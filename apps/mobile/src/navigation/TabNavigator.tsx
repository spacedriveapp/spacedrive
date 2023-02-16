import { BottomTabScreenProps, createBottomTabNavigator } from '@react-navigation/bottom-tabs';
import { CompositeScreenProps, NavigatorScreenParams } from '@react-navigation/native';
import { CirclesFour, Planet, ShareNetwork } from 'phosphor-react-native';
import React from 'react';
import tw from '~/lib/tailwind';
import type { HomeDrawerScreenProps } from './DrawerNavigator';
import NodesStack, { NodesStackParamList } from './tabs/NodesStack';
import OverviewStack, { OverviewStackParamList } from './tabs/OverviewStack';
import SpacesStack, { SpacesStackParamList } from './tabs/SpacesStack';

const Tab = createBottomTabNavigator<TabParamList>();

export default function TabNavigator() {
	return (
		<Tab.Navigator
			initialRouteName="OverviewStack"
			screenOptions={{
				headerShown: false,
				tabBarActiveTintColor: tw.color('accent'),
				tabBarInactiveTintColor: tw.color('ink'),
				tabBarStyle: {
					backgroundColor: tw.color('app'),
					borderTopColor: tw.color('app-shade')
				}
			}}
		>
			<Tab.Screen
				name="OverviewStack"
				component={OverviewStack}
				options={{
					tabBarIcon: ({ focused }) => (
						<Planet
							size={22}
							weight={focused ? 'bold' : 'regular'}
							color={focused ? tw.color('accent') : tw.color('ink')}
						/>
					),
					tabBarLabel: 'Overview',
					tabBarLabelStyle: tw`text-tiny font-semibold`
				}}
			/>
			<Tab.Screen
				name="NodesStack"
				component={NodesStack}
				options={{
					tabBarIcon: ({ focused }) => (
						<ShareNetwork
							size={22}
							weight={focused ? 'bold' : 'regular'}
							color={focused ? tw.color('accent') : tw.color('ink')}
						/>
					),
					tabBarLabel: 'Nodes',
					tabBarLabelStyle: tw`text-tiny font-semibold`
				}}
			/>
			<Tab.Screen
				name="SpacesStack"
				component={SpacesStack}
				options={{
					tabBarIcon: ({ focused }) => (
						<CirclesFour
							size={22}
							weight={focused ? 'bold' : 'regular'}
							color={focused ? tw.color('accent') : tw.color('ink')}
						/>
					),
					tabBarLabel: 'Spaces',
					tabBarLabelStyle: tw`text-tiny font-semibold`
				}}
			/>
		</Tab.Navigator>
	);
}

export type TabParamList = {
	OverviewStack: NavigatorScreenParams<OverviewStackParamList>;
	NodesStack: NavigatorScreenParams<NodesStackParamList>;
	SpacesStack: NavigatorScreenParams<SpacesStackParamList>;
};

export type TabScreenProps<Screen extends keyof TabParamList> = CompositeScreenProps<
	BottomTabScreenProps<TabParamList, Screen>,
	HomeDrawerScreenProps<'Home'>
>;
