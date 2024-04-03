import { createDrawerNavigator, DrawerScreenProps } from '@react-navigation/drawer';
import { CompositeScreenProps, NavigatorScreenParams } from '@react-navigation/native';
import { NativeStackScreenProps } from '@react-navigation/native-stack';
import DrawerContent from '~/components/drawer/DrawerContent';
import { tw } from '~/lib/tailwind';

import type { RootStackParamList } from '.';
import type { TabParamList } from './TabNavigator';
import TabNavigator from './TabNavigator';

const Drawer = createDrawerNavigator<DrawerNavParamList>();

export default function DrawerNavigator() {
	return (
		<Drawer.Navigator
			id="drawer"
			initialRouteName="Home"
			screenOptions={{
				headerShown: false,
				drawerStyle: {
					backgroundColor: tw.color('app-darkBox'),
					width: '70%',
					borderRightWidth: 1.5,
					borderRightColor: tw.color('app-cardborder')
				},
				overlayColor: 'transparent',
				drawerType: 'slide',
				swipeEdgeWidth: 50
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
	NativeStackScreenProps<RootStackParamList, 'Root'>
>;
