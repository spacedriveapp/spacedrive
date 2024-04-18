import { CompositeScreenProps } from '@react-navigation/native';
import { createNativeStackNavigator, NativeStackScreenProps } from '@react-navigation/native-stack';
import { useSharedValue } from 'react-native-reanimated';
import Header from '~/components/header/Header';
import CategoriesScreen from '~/screens/overview/Categories';
import OverviewScreen from '~/screens/overview/Overview';

import { TabScreenProps } from '../TabNavigator';

const Stack = createNativeStackNavigator<OverviewStackParamList>();
export default function OverviewStack() {
	const scrollY = useSharedValue(0);
	return (
		<Stack.Navigator initialRouteName="Overview">
			<Stack.Screen
				name="Overview"
				options={{
					header: () => (
						<Header scrollY={scrollY} showSearch showDrawer title="Overview" />
					)
				}}
			>
				{() => <OverviewScreen scrollY={scrollY} />}
			</Stack.Screen>
			<Stack.Screen
				name="Categories"
				options={{
					header: () => (
						<Header
							scrollY={scrollY}
							searchType="categories"
							navBack
							title="Categories"
						/>
					)
				}}
			>
				{() => <CategoriesScreen scrollY={scrollY} />}
			</Stack.Screen>
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
