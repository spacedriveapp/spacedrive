import { NavigatorScreenParams } from '@react-navigation/native';
import { createNativeStackNavigator, NativeStackScreenProps } from '@react-navigation/native-stack';
import NotFoundScreen from '~/screens/NotFound';

import DrawerNavigator, { DrawerNavParamList } from './DrawerNavigator';
import SearchStack, { SearchStackParamList } from './SearchStack';
import BackfillWaitingStack, { BackfillWaitingStackParamList } from './BackfillWaitingStack';

const Stack = createNativeStackNavigator<RootStackParamList>();
// This is the main navigator we nest everything under.
export default function RootNavigator() {
	return (
		<Stack.Navigator initialRouteName="Root">
			<Stack.Screen
				name="Root"
				component={DrawerNavigator}
				options={{ headerShown: false }}
			/>
			<Stack.Screen
				name="SearchStack"
				component={SearchStack}
				options={{ headerShown: false }}
			/>
			<Stack.Screen
				name="BackfillWaitingStack"
				component={BackfillWaitingStack}
				options={{ headerShown: false }}
			/>
			<Stack.Screen name="NotFound" component={NotFoundScreen} options={{ title: 'Oops!' }} />
		</Stack.Navigator>
	);
}

export type RootStackParamList = {
	Root: NavigatorScreenParams<DrawerNavParamList>;
	SearchStack: NavigatorScreenParams<SearchStackParamList>;
	BackfillWaitingStack: NavigatorScreenParams<BackfillWaitingStackParamList>;
	NotFound: undefined;
};

export type RootStackScreenProps<Screen extends keyof RootStackParamList> = NativeStackScreenProps<
	RootStackParamList,
	Screen
>;

// This declaration is used by useNavigation, Link, ref etc.
declare global {
	// eslint-disable-next-line @typescript-eslint/no-namespace
	namespace ReactNavigation {
		interface RootParamList extends RootStackParamList {}
	}
}
