import { NavigatorScreenParams } from '@react-navigation/native';
import { createStackNavigator, StackScreenProps } from '@react-navigation/stack';
import NotFoundScreen from '~/screens/NotFound';
import SearchScreen from '~/screens/Search';

import TabNavigator, { TabParamList } from './TabNavigator';

const Stack = createStackNavigator<RootStackParamList>();
// This is the main navigator we nest everything under.
export default function RootNavigator() {
	return (
		<Stack.Navigator initialRouteName="Root">
			<Stack.Screen name="Root" component={TabNavigator} options={{ headerShown: false }} />
			<Stack.Screen name="NotFound" component={NotFoundScreen} options={{ title: 'Oops!' }} />
			<Stack.Screen
				name="Search"
				component={SearchScreen}
				options={{ headerShown: false, animationEnabled: false }}
			/>
		</Stack.Navigator>
	);
}

export type RootStackParamList = {
	Root: NavigatorScreenParams<TabParamList>;
	NotFound: undefined;
	// Modals
	Search: undefined;
};

export type RootStackScreenProps<Screen extends keyof RootStackParamList> = StackScreenProps<
	RootStackParamList,
	Screen
>;

// This declaration is used by useNavigation, Link, ref etc.
declare global {
	// eslint-disable-next-line @typescript-eslint/no-namespace
	namespace ReactNavigation {
		interface RootParamList extends RootStackParamList { }
	}
}
