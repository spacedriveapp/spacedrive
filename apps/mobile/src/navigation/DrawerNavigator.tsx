import { createDrawerNavigator } from '@react-navigation/drawer';

import ContentScreen from '../screens/Content';
import LocationScreen from '../screens/Location';
import OverviewScreen from '../screens/Overview';
import PhotosScreen from '../screens/Photos';
import TagScreen from '../screens/Tag';
import SettingsScreen from '../screens/settings/Settings';
import { HomeDrawerParamList } from '../types/navigation';

const Drawer = createDrawerNavigator<HomeDrawerParamList>();

// TODO: Implement CustomDrawerContent
// TODO: Implement Animated Drawer (maybe scale down + blur the screen when drawer is open)
// TODO: Implement Animated Height to expand Locations & Tags
// TODO: Custom Header with Search and Button to open drawer

export default function DrawerNavigator() {
	return (
		<Drawer.Navigator>
			<Drawer.Screen name="Overview" component={OverviewScreen} />
			<Drawer.Screen name="Content" component={ContentScreen} />
			<Drawer.Screen name="Photos" component={PhotosScreen} />
			<Drawer.Screen name="Location" component={LocationScreen} />
			<Drawer.Screen name="Tag" component={TagScreen} />
			<Drawer.Screen name="Settings" component={SettingsScreen} />
		</Drawer.Navigator>
	);
}
