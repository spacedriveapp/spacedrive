import { CompositeScreenProps } from '@react-navigation/native';
import { createNativeStackNavigator, NativeStackScreenProps } from '@react-navigation/native-stack';
import Header from '~/components/header/Header';
import SearchHeader from '~/components/header/SearchHeader';
import CategoriesScreen from '~/screens/overview/Categories';
import OverviewScreen from '~/screens/overview/Overview';

import { TabScreenProps } from '../TabNavigator';

const Stack = createNativeStackNavigator<OverviewStackParamList>();

export default function OverviewStack() {
	return (
		<Stack.Navigator
			screenOptions={{
				fullScreenGestureEnabled: true
			}}
			initialRouteName="Overview"
		>
			<Stack.Screen
				name="Overview"
				component={OverviewScreen}
				options={({ route }) => ({
					header: () => <Header search route={route} />
				})}
			/>
			<Stack.Screen
				name="Categories"
				component={CategoriesScreen}
				options={({ route }) => ({
					header: () => <SearchHeader kind="categories" route={route} />
				})}
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
