import { NavigatorScreenParams } from '@react-navigation/native';
import { NativeStackScreenProps, createNativeStackNavigator } from '@react-navigation/native-stack';

import NotFoundScreen from '../screens/NotFound';
import SettingsScreen from '../screens/modals/settings/Settings';
import type { DrawerNavParamList } from './DrawerNavigator';
import DrawerNavigator from './DrawerNavigator';

const Stack = createNativeStackNavigator<RootStackParamList>();

// This is the main navigator we nest everything under.
export default function RootNavigator() {
	return (
		<Stack.Navigator initialRouteName="Root">
			<Stack.Screen name="Root" component={DrawerNavigator} options={{ headerShown: false }} />
			<Stack.Screen name="NotFound" component={NotFoundScreen} options={{ title: 'Oops!' }} />
			<Stack.Group screenOptions={{ presentation: 'modal' }}>
				<Stack.Screen name="Settings" component={SettingsScreen} />
			</Stack.Group>
		</Stack.Navigator>
	);
}

export type RootStackParamList = {
	Root: NavigatorScreenParams<DrawerNavParamList>;
	NotFound: undefined;
	// Modals
	Settings: undefined;
};

export type RootStackScreenProps<Screen extends keyof RootStackParamList> = NativeStackScreenProps<
	RootStackParamList,
	Screen
>;
