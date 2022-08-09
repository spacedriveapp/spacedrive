import { CompositeScreenProps } from '@react-navigation/native';
import { NativeStackScreenProps, createNativeStackNavigator } from '@react-navigation/native-stack';

import PhotosScreen from '../../screens/Photos';
import { SharedScreens, SharedScreensParamList } from '../SharedScreens';
import { TabScreenProps } from '../TabNavigator';

const Stack = createNativeStackNavigator<PhotosStackParamList>();

export default function PhotosStack() {
	return (
		<Stack.Navigator
			initialRouteName="Photos"
			screenOptions={{
				headerStyle: { backgroundColor: '#08090D' },
				headerTintColor: '#fff'
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
		NativeStackScreenProps<PhotosStackParamList, Screen>,
		TabScreenProps<'PhotosStack'>
	>;
