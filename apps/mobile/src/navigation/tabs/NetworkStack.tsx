import { CompositeScreenProps } from '@react-navigation/native';
import { createNativeStackNavigator, NativeStackScreenProps } from '@react-navigation/native-stack';
import Header from '~/components/header/Header';
import NetworkScreen from '~/screens/network/Network';

import { TabScreenProps } from '../TabNavigator';

const Stack = createNativeStackNavigator<NetworkStackParamList>();

export default function NetworkStack() {
	return (
		<Stack.Navigator initialRouteName="Network">
			<Stack.Screen
				name="Network"
				component={NetworkScreen}
				options={({ route }) => ({
					header: () => <Header search route={route} />
				})}
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
