import { CompositeScreenProps } from '@react-navigation/native';
import { NativeStackScreenProps, createNativeStackNavigator } from '@react-navigation/native-stack';

import OverviewScreen from '../../screens/Overview';
import { SharedScreens, SharedScreensParamList } from '../SharedScreens';
import { TabScreenProps } from '../TabNavigator';

const Stack = createNativeStackNavigator<OverviewStackParamList>();

export default function OverviewStack() {
	return (
		<Stack.Navigator
			initialRouteName="Overview"
			screenOptions={{
				headerStyle: { backgroundColor: '#08090D' },
				headerTintColor: '#fff'
			}}
		>
			<Stack.Screen name="Overview" component={OverviewScreen} />
			{SharedScreens(Stack as any)}
		</Stack.Navigator>
	);
}

export type OverviewStackParamList = {
	Overview: undefined;
} & SharedScreensParamList;

export type OverviewStackScreenProps<Screen extends keyof OverviewStackParamList> =
	CompositeScreenProps<
		NativeStackScreenProps<OverviewStackParamList, Screen>,
		TabScreenProps<'OverviewStack'>
	>;
