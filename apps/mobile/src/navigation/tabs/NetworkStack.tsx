import { CompositeScreenProps } from '@react-navigation/native';
import { createNativeStackNavigator } from '@react-navigation/native-stack';
import { StackScreenProps } from '@react-navigation/stack';
import Header from '~/components/header/Header';
import NetworkScreen from '~/screens/network';

import { TabScreenProps } from '../TabNavigator';

const Stack = createNativeStackNavigator<NetworkStackParamList>();

export default function NetworkStack() {
	return (
		<Stack.Navigator initialRouteName="Network">
			<Stack.Screen
				name="Network"
				component={NetworkScreen}
				options={{ header: () => <Header title="Network" /> }}
			/>
		</Stack.Navigator>
	);
}

export type NetworkStackParamList = {
	Network: undefined;
};

export type NetworkStackScreenProps<Screen extends keyof NetworkStackParamList> =
	CompositeScreenProps<
		StackScreenProps<NetworkStackParamList, Screen>,
		TabScreenProps<'NetworkStack'>
	>;
