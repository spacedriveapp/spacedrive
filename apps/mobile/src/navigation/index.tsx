import { NavigatorScreenParams } from '@react-navigation/native';
import { StackScreenProps, createStackNavigator } from '@react-navigation/stack';
import tw from '~/lib/tailwind';
import NotFoundScreen from '~/screens/NotFound';
import SearchScreen from '~/screens/Search';

import type { DrawerNavParamList } from './DrawerNavigator';
import DrawerNavigator from './DrawerNavigator';
import SettingsNavigator, { SettingsStackParamList } from './SettingsNavigator';

const Stack = createStackNavigator<RootStackParamList>();

// This is the main navigator we nest everything under.
export default function RootNavigator() {
	return (
		<Stack.Navigator initialRouteName="Root">
			<Stack.Screen name="Root" component={DrawerNavigator} options={{ headerShown: false }} />
			<Stack.Screen name="NotFound" component={NotFoundScreen} options={{ title: 'Oops!' }} />
			<Stack.Screen
				name="Search"
				component={SearchScreen}
				options={{ headerShown: false, animationEnabled: false }}
			/>
			{/* Modals */}
			<Stack.Group
				screenOptions={{
					headerShown: false,
					presentation: 'modal',
					headerBackTitleVisible: false,
					headerStyle: tw`bg-app`,
					headerTintColor: tw.color('ink'),
					headerTitleStyle: tw`text-base`,
					headerBackTitleStyle: tw`text-base`
					// headerShadowVisible: false // will disable the white line under
				}}
			>
				<Stack.Screen name="Settings" component={SettingsNavigator} />
			</Stack.Group>
		</Stack.Navigator>
	);
}

export type RootStackParamList = {
	Root: NavigatorScreenParams<DrawerNavParamList>;
	NotFound: undefined;
	// Modals
	Search: undefined;
	Settings: NavigatorScreenParams<SettingsStackParamList>;
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
