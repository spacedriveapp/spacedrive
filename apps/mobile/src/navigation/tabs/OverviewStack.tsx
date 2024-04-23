import { CompositeScreenProps } from '@react-navigation/native';
import { createNativeStackNavigator, NativeStackScreenProps } from '@react-navigation/native-stack';
import CategoriesScreen from '~/screens/overview/Categories';
import OverviewScreen from '~/screens/overview/Overview';

import { TabScreenProps } from '../TabNavigator';

const Stack = createNativeStackNavigator<OverviewStackParamList>();
export default function OverviewStack() {
	return (
		<Stack.Navigator
		 screenOptions={{
			headerShown: false
		 }}>
			<Stack.Screen
				name="Overview"
				component={OverviewScreen}
			/>
			<Stack.Screen
				name="Categories"
				component={CategoriesScreen}
			/>
		</Stack.Navigator>
	);
}

export type OverviewStackParamList = {
	Overview: undefined;
	Categories: undefined;
};

export type OverviewStackScreenProps<Screen extends keyof OverviewStackParamList> =
	CompositeScreenProps<
		NativeStackScreenProps<OverviewStackParamList, Screen>,
		TabScreenProps<'OverviewStack'>
	>;
