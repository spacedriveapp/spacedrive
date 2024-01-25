import { BottomTabScreenProps, createBottomTabNavigator } from '@react-navigation/bottom-tabs';
import { CompositeScreenProps, NavigatorScreenParams } from '@react-navigation/native';
import { StackScreenProps } from '@react-navigation/stack';
import { BlurView } from 'expo-blur';
import { CirclesFour, FolderOpen, Gear, Planet } from 'phosphor-react-native';
import { StyleSheet } from 'react-native';
import { tw } from '~/lib/tailwind';

import { RootStackParamList } from '.';
import BrowseStack, { BrowseStackParamList } from './tabs/BrowseStack';
import NetworkStack, { NetworkStackParamList } from './tabs/NetworkStack';
import OverviewStack, { OverviewStackParamList } from './tabs/OverviewStack';
import SettingsStack, { SettingsStackParamList } from './tabs/SettingsStack';

const Tab = createBottomTabNavigator<TabParamList>();

export default function TabNavigator() {
	return (
		<Tab.Navigator
			id="tab"
			initialRouteName="OverviewStack"
			screenOptions={{
				tabBarStyle: {
					position: 'absolute',
					backgroundColor: tw.color('mobile-navtab'),
					borderTopWidth: 1,
					borderTopColor: tw.color('app-line/50')
				},
				tabBarBackground: () => (
					<BlurView tint="dark" intensity={50} style={StyleSheet.absoluteFill} />
				),
				headerShown: false,
				tabBarActiveTintColor: tw.color('accent'),
				tabBarInactiveTintColor: tw.color('ink-faint')
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
							color={focused ? tw.color('accent') : tw.color('ink-dull')}
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
							color={focused ? tw.color('accent') : tw.color('ink-dull')}
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
							color={focused ? tw.color('accent') : tw.color('ink-dull')}
						/>
					),
					tabBarTestID: 'browse-tab',
					tabBarLabel: 'Browse',
					tabBarLabelStyle: tw`text-[10px] font-semibold`
				}}
			/>
			<Tab.Screen
				name="SettingsStack"
				component={SettingsStack}
				options={{
					tabBarIcon: ({ focused }) => (
						<Gear
							size={22}
							weight={focused ? 'bold' : 'regular'}
							color={focused ? tw.color('accent') : tw.color('ink-dull')}
						/>
					),
					tabBarTestID: 'settings-tab',
					tabBarLabel: 'Settings',
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
	SettingsStack: NavigatorScreenParams<SettingsStackParamList>;
};

export type TabScreenProps<Screen extends keyof TabParamList> = CompositeScreenProps<
	BottomTabScreenProps<TabParamList, Screen>,
	StackScreenProps<RootStackParamList, 'Root'>
>;
