import { createDrawerNavigator } from '@react-navigation/drawer';

import DrawerContent from '../components/drawer/DrawerContent';
import LocationScreen from '../screens/Location';
import TagScreen from '../screens/Tag';
import SettingsScreen from '../screens/settings/Settings';
import { DrawerNavParamList } from '../types/navigation';
import BottomTabNavigator from './BottomTabNavigator';

const Drawer = createDrawerNavigator<DrawerNavParamList>();

export default function DrawerNavigator() {
	return (
		<Drawer.Navigator
			initialRouteName="Home"
			screenOptions={{
				headerShown: false,
				drawerStyle: {
					backgroundColor: '#08090D',
					width: '75%'
				},
				overlayColor: 'transparent'
				// drawerHideStatusBarOnOpen: true,
				// drawerStatusBarAnimation: 'slide'
			}}
			drawerContent={(props) => <DrawerContent {...(props as any)} />}
		>
			<Drawer.Screen name="Home" component={BottomTabNavigator} />
			<Drawer.Screen name="Location" component={LocationScreen} />
			<Drawer.Screen name="Tag" component={TagScreen} />
			<Drawer.Screen name="Settings" component={SettingsScreen} />
		</Drawer.Navigator>
	);
}
