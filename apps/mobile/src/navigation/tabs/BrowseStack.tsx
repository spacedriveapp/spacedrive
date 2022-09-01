import Header from '@app/components/header/Header';
import tw from '@app/lib/tailwind';
import BrowseScreen from '@app/screens/Browse';
import { CompositeScreenProps } from '@react-navigation/native';
import { StackScreenProps, createStackNavigator } from '@react-navigation/stack';

import { SharedScreens, SharedScreensParamList } from '../SharedScreens';
import { TabScreenProps } from '../TabNavigator';

const Stack = createStackNavigator<BrowseStackParamList>();

export default function BrowseStack() {
	return (
		<Stack.Navigator
			initialRouteName="Browse"
			screenOptions={{
				headerStyle: { backgroundColor: tw.color('gray-650') },
				headerTintColor: tw.color('gray-200'),
				headerTitleStyle: tw`text-base`,
				headerBackTitleStyle: tw`text-base`
			}}
		>
			<Stack.Screen name="Browse" component={BrowseScreen} options={{ header: Header }} />
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
