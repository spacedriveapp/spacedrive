import { CompositeScreenProps } from '@react-navigation/native';
import { NativeStackScreenProps, createNativeStackNavigator } from '@react-navigation/native-stack';

import SpacesScreen from '../../screens/Spaces';
import { SharedScreens, SharedScreensParamList } from '../SharedScreens';
import { TabScreenProps } from '../TabNavigator';

const Stack = createNativeStackNavigator<SpacesStackParamList>();

export default function SpacesStack() {
	return (
		<Stack.Navigator
			initialRouteName="Spaces"
			screenOptions={{
				headerStyle: { backgroundColor: '#08090D' },
				headerTintColor: '#fff'
			}}
		>
			<Stack.Screen name="Spaces" component={SpacesScreen} />
			{SharedScreens(Stack as any)}
		</Stack.Navigator>
	);
}

export type SpacesStackParamList = {
	Spaces: undefined;
} & SharedScreensParamList;

export type SpacesStackScreenProps<Screen extends keyof SpacesStackParamList> =
	CompositeScreenProps<
		NativeStackScreenProps<SpacesStackParamList, Screen>,
		TabScreenProps<'SpacesStack'>
	>;
