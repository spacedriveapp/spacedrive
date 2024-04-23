import { CompositeScreenProps } from '@react-navigation/native';
import { createNativeStackNavigator, NativeStackScreenProps } from '@react-navigation/native-stack';
import NetworkScreen from '~/screens/network/Network';

import { TabScreenProps } from '../TabNavigator';

const Stack = createNativeStackNavigator<NetworkStackParamList>();

export default function NetworkStack() {
	return (
		<Stack.Navigator screenOptions={{
			headerShown: false
		}} initialRouteName="Network">
			<Stack.Screen
				name="Network"
				component={NetworkScreen}
			/>
		</Stack.Navigator>
	);
}

export type NetworkStackParamList = {
	Network: undefined;
};

export type NetworkStackScreenProps<Screen extends keyof NetworkStackParamList> =
	CompositeScreenProps<
		NativeStackScreenProps<NetworkStackParamList, Screen>,
		TabScreenProps<'NetworkStack'>
	>;
