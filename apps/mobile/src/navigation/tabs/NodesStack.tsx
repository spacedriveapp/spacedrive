import { CompositeScreenProps } from '@react-navigation/native';
import { StackScreenProps, createStackNavigator } from '@react-navigation/stack';
import Header from '~/components/header/Header';
import { tw } from '~/lib/tailwind';
import NodesScreen from '~/screens/Nodes';
import { SharedScreens, SharedScreensParamList } from '../SharedScreens';
import { TabScreenProps } from '../TabNavigator';

const Stack = createStackNavigator<NodesStackParamList>();

export default function NodesStack() {
	return (
		<Stack.Navigator
			initialRouteName="Nodes"
			screenOptions={{
				headerStyle: { backgroundColor: tw.color('app-box') },
				headerTintColor: tw.color('ink'),
				headerTitleStyle: tw`text-base`,
				headerBackTitleStyle: tw`text-base`
			}}
		>
			<Stack.Screen name="Nodes" component={NodesScreen} options={{ header: Header }} />
			{SharedScreens(Stack as any)}
		</Stack.Navigator>
	);
}

export type NodesStackParamList = {
	Nodes: undefined;
} & SharedScreensParamList;

export type NodesStackScreenProps<Screen extends keyof NodesStackParamList> = CompositeScreenProps<
	StackScreenProps<NodesStackParamList, Screen>,
	TabScreenProps<'NodesStack'>
>;
