import { CompositeScreenProps } from '@react-navigation/native';
import {
	HeaderStyleInterpolators,
	StackScreenProps,
	TransitionPresets,
	createStackNavigator
} from '@react-navigation/stack';

import PhotosScreen from '../../screens/Photos';
import { SharedScreens, SharedScreensParamList } from '../SharedScreens';
import { TabScreenProps } from '../TabNavigator';

const Stack = createStackNavigator<PhotosStackParamList>();

export default function PhotosStack() {
	return (
		<Stack.Navigator
			initialRouteName="Photos"
			screenOptions={{
				headerStyle: { backgroundColor: '#08090D' },
				headerTintColor: '#fff',
				headerStyleInterpolator: HeaderStyleInterpolators.forUIKit,
				...TransitionPresets.DefaultTransition
			}}
		>
			<Stack.Screen name="Photos" component={PhotosScreen} />
			{SharedScreens(Stack as any)}
		</Stack.Navigator>
	);
}

export type PhotosStackParamList = {
	Photos: undefined;
} & SharedScreensParamList;

export type PhotosStackScreenProps<Screen extends keyof PhotosStackParamList> =
	CompositeScreenProps<
		StackScreenProps<PhotosStackParamList, Screen>,
		TabScreenProps<'PhotosStack'>
	>;
