import { CompositeScreenProps } from '@react-navigation/native';
import { createNativeStackNavigator, NativeStackScreenProps } from '@react-navigation/native-stack';
import Header from '~/components/header/Header';
import CategoriesScreen from '~/screens/overview/Categories';
import OverviewScreen from '~/screens/overview/index';

import { TabScreenProps } from '../TabNavigator';

const Stack = createNativeStackNavigator<OverviewStackParamList>();

export default function OverviewStack() {
	return (
		<Stack.Navigator initialRouteName="Overview">
			<Stack.Screen
				name="Overview"
				component={OverviewScreen}
				options={{ header: () => <Header title="Overview" /> }}
			/>
			<Stack.Screen
				name="Categories"
				component={CategoriesScreen}
				options={{
					header: () => <Header searchType="categories" navBack title="Categories" />
				}}
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
