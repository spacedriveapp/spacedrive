import { CompositeScreenProps } from '@react-navigation/native';
import {
	HeaderStyleInterpolators,
	StackScreenProps,
	TransitionPresets,
	createStackNavigator
} from '@react-navigation/stack';

import BrowseScreen from '../../screens/Browse';
import { SharedScreens, SharedScreensParamList } from '../SharedScreens';
import { TabScreenProps } from '../TabNavigator';

const Stack = createStackNavigator<BrowseStackParamList>();

export default function BrowseStack() {
	return (
		<Stack.Navigator
			initialRouteName="Browse"
			screenOptions={{
				headerStyle: { backgroundColor: '#08090D' },
				headerTintColor: '#fff',
				headerStyleInterpolator: HeaderStyleInterpolators.forUIKit,
				...TransitionPresets.DefaultTransition
			}}
		>
			<Stack.Screen name="Browse" component={BrowseScreen} />
			{SharedScreens(Stack as any)}
		</Stack.Navigator>
	);
}

export type BrowseStackParamList = {
	Browse: undefined;
} & SharedScreensParamList;

export type BrowseStackScreenProps<Screen extends keyof BrowseStackParamList> =
	CompositeScreenProps<
		StackScreenProps<BrowseStackParamList, Screen>,
		TabScreenProps<'BrowseStack'>
	>;
