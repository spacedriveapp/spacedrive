import { BottomTabScreenProps, createBottomTabNavigator } from '@react-navigation/bottom-tabs';
import { CompositeScreenProps, NavigatorScreenParams } from '@react-navigation/native';
import { StackScreenProps } from '@react-navigation/stack';
import { BlurView } from 'expo-blur';
import { useEffect, useRef, useState } from 'react';
import { StyleSheet } from 'react-native';
import { TouchableWithoutFeedback } from 'react-native-gesture-handler';
import Rive, { RiveRef } from 'rive-react-native';
import { Style } from 'twrnc/dist/esm/types';
import { tw } from '~/lib/tailwind';

import { RootStackParamList } from '.';
import BrowseStack, { BrowseStackParamList } from './tabs/BrowseStack';
import NetworkStack, { NetworkStackParamList } from './tabs/NetworkStack';
import OverviewStack, { OverviewStackParamList } from './tabs/OverviewStack';
import SettingsStack, { SettingsStackParamList } from './tabs/SettingsStack';

const Tab = createBottomTabNavigator<TabParamList>();

//TouchableWithoutFeedback is used to prevent Android ripple effect
//State is being used to control the animation and make Rive work
//Tab.Screen listeners are needed because if a user taps on the tab text only, the animation won't play
//This may be revisted in the future to update accordingly

export default function TabNavigator() {
	const [activeIndex, setActiveIndex] = useState(0);
	const TabScreens: {
		name: keyof TabParamList;
		component: () => React.JSX.Element;
		icon: React.ReactNode;
		label: string;
		labelStyle: Style;
		testID?: string;
	}[] = [
		{
			name: 'OverviewStack',
			component: OverviewStack,
			icon: (
				<TabBarButton
					resourceName="tabs"
					animationName="animate"
					artboardName="overview"
					style={{ width: 28 }}
					active={activeIndex === 0}
				/>
			),
			label: 'Overview',
			labelStyle: tw`text-[10px] font-semibold`
		},
		{
			name: 'NetworkStack',
			component: NetworkStack,
			icon: (
				<TabBarButton
					resourceName="tabs"
					animationName="animate"
					artboardName="network"
					style={{ width: 18, maxHeight: 23 }}
					active={activeIndex === 1}
				/>
			),
			label: 'Network',
			labelStyle: tw`text-[10px] font-semibold`
		},
		{
			name: 'BrowseStack',
			component: BrowseStack,
			icon: (
				<TabBarButton
					resourceName="tabs"
					animationName="animate"
					artboardName="browse"
					style={{ width: 20 }}
					active={activeIndex === 2}
				/>
			),
			label: 'Browse',
			labelStyle: tw`text-[10px] font-semibold`,
			testID: 'browse-tab'
		},
		{
			name: 'SettingsStack',
			component: SettingsStack,
			icon: (
				<TabBarButton
					resourceName="tabs"
					animationName="animate"
					artboardName="settings"
					style={{ width: 19 }}
					active={activeIndex === 3}
				/>
			),
			label: 'Settings',
			labelStyle: tw`text-[10px] font-semibold`
		}
	];
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
			{TabScreens.map((screen, index) => (
				<Tab.Screen
					key={screen.name + index}
					name={screen.name}
					component={screen.component}
					options={{
						tabBarLabel: screen.label,
						tabBarLabelStyle: screen.labelStyle,
						tabBarIcon: () => (
							<TouchableWithoutFeedback>{screen.icon}</TouchableWithoutFeedback>
						),
						tabBarTestID: screen.testID
					}}
					listeners={() => ({
						tabPress: () => {
							setActiveIndex(index);
						}
					})}
				/>
			))}
		</Tab.Navigator>
	);
}

interface TabBarButtonProps {
	active: boolean;
	resourceName: string;
	animationName: string;
	artboardName: string;
	style?: any;
}

const TabBarButton = ({
	active,
	resourceName,
	animationName,
	artboardName,
	style
}: TabBarButtonProps) => {
	const ref = useRef<RiveRef>(null);
	useEffect(() => {
		if (active && ref.current) {
			ref.current?.play();
		} else ref.current?.stop();
	}, [active]);
	return (
		<Rive
			ref={ref}
			autoplay={active}
			resourceName={resourceName}
			animationName={animationName}
			artboardName={artboardName}
			style={style}
		/>
	);
};

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
