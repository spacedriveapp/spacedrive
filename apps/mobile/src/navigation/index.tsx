import NotFoundScreen from '@app/screens/NotFound';
import SearchScreen from '@app/screens/modals/Search';
import SettingsScreen from '@app/screens/modals/settings/Settings';
import { NavigatorScreenParams } from '@react-navigation/native';
import { StackScreenProps, createStackNavigator } from '@react-navigation/stack';

import type { DrawerNavParamList } from './DrawerNavigator';
import DrawerNavigator from './DrawerNavigator';

const Stack = createStackNavigator<RootStackParamList>();

// This is the main navigator we nest everything under.
export default function RootNavigator() {
	return (
		<Stack.Navigator initialRouteName="Root">
			<Stack.Screen name="Root" component={DrawerNavigator} options={{ headerShown: false }} />
			<Stack.Screen name="NotFound" component={NotFoundScreen} options={{ title: 'Oops!' }} />
			<Stack.Screen name="Search" component={SearchScreen} options={{ headerShown: false }} />
			{/* Modals */}
			<Stack.Group
				screenOptions={{
					presentation: 'modal',
					headerBackTitleVisible: false,
					headerStyle: { backgroundColor: '#08090D' },
					// headerShadowVisible: false,
					headerTintColor: '#fff'
				}}
			>
				<Stack.Screen name="Settings" component={SettingsScreen} />
			</Stack.Group>
		</Stack.Navigator>
	);
}

export type RootStackParamList = {
	Root: NavigatorScreenParams<DrawerNavParamList>;
	NotFound: undefined;
	// Modals
	Search: undefined;
	Settings: undefined;
};

export type RootStackScreenProps<Screen extends keyof RootStackParamList> = StackScreenProps<
	RootStackParamList,
	Screen
>;

// This declaration is used by useNavigation, Link, ref etc.
declare global {
	// eslint-disable-next-line @typescript-eslint/no-namespace
	namespace ReactNavigation {
		// eslint-disable-next-line @typescript-eslint/no-empty-interface
		interface RootParamList extends RootStackParamList {}
	}
}
