import { createNativeStackNavigator } from '@react-navigation/native-stack';

import NotFoundScreen from '../screens/NotFound';
import ModalScreen from '../screens/modals/ModalScreen';
import { RootStackParamList } from '../types/navigation';
import DrawerNavigator from './DrawerNavigator';

const Stack = createNativeStackNavigator<RootStackParamList>();

// This is the main navigator we nest everything under.
export default function RootNavigator() {
	return (
		<Stack.Navigator>
			<Stack.Screen name="Root" component={DrawerNavigator} options={{ headerShown: false }} />
			<Stack.Screen name="NotFound" component={NotFoundScreen} options={{ title: 'Oops!' }} />
			<Stack.Group screenOptions={{ presentation: 'modal' }}>
				<Stack.Screen name="Modal" component={ModalScreen} />
			</Stack.Group>
		</Stack.Navigator>
	);
}
