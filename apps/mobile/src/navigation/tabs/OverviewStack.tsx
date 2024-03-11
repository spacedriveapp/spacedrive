import { CompositeScreenProps } from '@react-navigation/native';
import { createStackNavigator, StackScreenProps } from '@react-navigation/stack';
import Header from '~/components/header/Header';

import OverviewScreen from '../../screens/Overview';
import { TabScreenProps } from '../TabNavigator';

const Stack = createStackNavigator<OverviewStackParamList>();

export default function OverviewStack() {
	return (
		<Stack.Navigator initialRouteName="Overview">
			<Stack.Screen
				name="Overview"
				component={OverviewScreen}
				options={{ header: () => <Header title="Overview" /> }}
			/>
		</Stack.Navigator>
	);
}

export type OverviewStackParamList = {
	Overview: undefined;
};

export type OverviewStackScreenProps<Screen extends keyof OverviewStackParamList> =
	CompositeScreenProps<
		StackScreenProps<OverviewStackParamList, Screen>,
		TabScreenProps<'OverviewStack'>
	>;
