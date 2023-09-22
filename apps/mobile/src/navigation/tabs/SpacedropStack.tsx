import { CompositeScreenProps } from '@react-navigation/native';
import { createStackNavigator, StackScreenProps } from '@react-navigation/stack';
import Header from '~/components/header/Header';
import { tw } from '~/lib/tailwind';
import SpacedropScreen from '~/screens/Spacedrop';

import { SharedScreens, SharedScreensParamList } from '../SharedScreens';
import { TabScreenProps } from '../TabNavigator';

const Stack = createStackNavigator<SpacedropStackParamList>();

export default function SpacedropStack() {
	return (
		<Stack.Navigator
			initialRouteName="Spacedrop"
			screenOptions={{
				headerStyle: { backgroundColor: tw.color('app-box') },
				headerTintColor: tw.color('ink'),
				headerTitleStyle: tw`text-base`,
				headerBackTitleStyle: tw`text-base`
			}}
		>
			<Stack.Screen
				name="Spacedrop"
				component={SpacedropScreen}
				options={{ header: Header }}
			/>
			{SharedScreens(Stack as any)}
		</Stack.Navigator>
	);
}

export type SpacedropStackParamList = {
	Spacedrop: undefined;
} & SharedScreensParamList;

export type SpacedropStackScreenProps<Screen extends keyof SpacedropStackParamList> =
	CompositeScreenProps<
		StackScreenProps<SpacedropStackParamList, Screen>,
		TabScreenProps<'SpacedropStack'>
	>;
