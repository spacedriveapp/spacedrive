import { CompositeScreenProps } from '@react-navigation/native';
import { StackScreenProps, TransitionPresets, createStackNavigator } from '@react-navigation/stack';
import tw from '~/lib/tailwind';

import Header from '../../components/header/Header';
import SpacesScreen from '../../screens/Spaces';
import { SharedScreens, SharedScreensParamList } from '../SharedScreens';
import { TabScreenProps } from '../TabNavigator';

const Stack = createStackNavigator<SpacesStackParamList>();

export default function SpacesStack() {
	return (
		<Stack.Navigator
			initialRouteName="Spaces"
			screenOptions={{
				headerStyle: { backgroundColor: tw.color('app-box') },
				headerTintColor: tw.color('ink'),
				headerTitleStyle: tw`text-base`,
				headerBackTitleStyle: tw`text-base`
			}}
		>
			<Stack.Screen name="Spaces" component={SpacesScreen} options={{ header: Header }} />
			{SharedScreens(Stack as any)}
		</Stack.Navigator>
	);
}

export type SpacesStackParamList = {
	Spaces: undefined;
} & SharedScreensParamList;

export type SpacesStackScreenProps<Screen extends keyof SpacesStackParamList> =
	CompositeScreenProps<
		StackScreenProps<SpacesStackParamList, Screen>,
		TabScreenProps<'SpacesStack'>
	>;
