import { DrawerScreenProps, createDrawerNavigator } from '@react-navigation/drawer';
import { CompositeScreenProps, NavigatorScreenParams } from '@react-navigation/native';
import { StackScreenProps } from '@react-navigation/stack';

import type { RootStackParamList } from '.';
import DrawerContent from '../components/drawer/DrawerContent';
import type { TabParamList } from './TabNavigator';
import TabNavigator from './TabNavigator';

const Drawer = createDrawerNavigator<DrawerNavParamList>();

export default function DrawerNavigator() {
	return (
		<Drawer.Navigator
			initialRouteName="Home"
			screenOptions={({ route }) => {
				return {
					headerShown: false,
					drawerStyle: {
						backgroundColor: '#08090D',
						width: '75%'
					},
					overlayColor: 'transparent',
					drawerType: 'slide'
					// swipeEnabled: false
					// drawerHideStatusBarOnOpen: true,
					// drawerStatusBarAnimation: 'slide'
				};
			}}
			drawerContent={(props) => <DrawerContent {...(props as any)} />}
		>
			<Drawer.Screen name="Home" component={TabNavigator} />
		</Drawer.Navigator>
	);
}

export type DrawerNavParamList = {
	Home: NavigatorScreenParams<TabParamList>;
};

export type HomeDrawerScreenProps<Screen extends keyof DrawerNavParamList> = CompositeScreenProps<
	DrawerScreenProps<DrawerNavParamList, Screen>,
	StackScreenProps<RootStackParamList, 'Root'>
>;
