import { BottomTabScreenProps, createBottomTabNavigator } from '@react-navigation/bottom-tabs';
import { CompositeScreenProps, NavigatorScreenParams } from '@react-navigation/native';
import { StackScreenProps } from '@react-navigation/stack';
import { BlurView } from 'expo-blur';
import { CirclesFour, FolderOpen, Planet } from 'phosphor-react-native';
import { StyleSheet } from 'react-native';
import { tw } from '~/lib/tailwind';

import { RootStackParamList } from '.';
import BrowseStack, { BrowseStackParamList } from './tabs/BrowseStack';
import NetworkStack, { NetworkStackParamList } from './tabs/NetworkStack';
import OverviewStack, { OverviewStackParamList } from './tabs/OverviewStack';

const Tab = createBottomTabNavigator<TabParamList>();

export default function TabNavigator() {
	return (
		<Tab.Navigator
			id="tab"
			initialRouteName="OverviewStack"
			screenOptions={{
				tabBarStyle: {
					position: 'absolute',
					// backgroundColor: 'transparent',
					borderTopColor: tw.color('app')
				},
				tabBarBackground: () => (
					<BlurView tint="dark" intensity={100} style={StyleSheet.absoluteFill} />
				),
				headerShown: false,
				tabBarActiveTintColor: tw.color('accent'),
				tabBarInactiveTintColor: tw.color('ink')
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
					tabBarLabelStyle: tw`text-[10px] font-semibold`
				}}
			/>
			<Tab.Screen
				name="NetworkStack"
				component={NetworkStack}
				options={{
					tabBarIcon: ({ focused }) => (
						<CirclesFour
							size={22}
							weight={focused ? 'bold' : 'regular'}
							color={focused ? tw.color('accent') : tw.color('ink')}
						/>
					),
					tabBarLabel: 'Network',
					tabBarLabelStyle: tw`text-[10px] font-semibold`
				}}
			/>
			<Tab.Screen
				name="BrowseStack"
				component={BrowseStack}
				options={{
					tabBarIcon: ({ focused }) => (
						<FolderOpen
							size={22}
							weight={focused ? 'bold' : 'regular'}
							color={focused ? tw.color('accent') : tw.color('ink')}
						/>
					),
					tabBarTestID: 'browse-tab',
					tabBarLabel: 'Browse',
					tabBarLabelStyle: tw`text-[10px] font-semibold`
				}}
			/>
		</Tab.Navigator>
	);
}

export type TabParamList = {
	OverviewStack: NavigatorScreenParams<OverviewStackParamList>;
	NetworkStack: NavigatorScreenParams<NetworkStackParamList>;
	BrowseStack: NavigatorScreenParams<BrowseStackParamList>;
};

export type TabScreenProps<Screen extends keyof TabParamList> = CompositeScreenProps<
	BottomTabScreenProps<TabParamList, Screen>,
	StackScreenProps<RootStackParamList, 'Root'>
>;
