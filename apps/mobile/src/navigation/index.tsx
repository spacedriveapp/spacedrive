import { FontAwesome } from '@expo/vector-icons';
import { createBottomTabNavigator } from '@react-navigation/bottom-tabs';
import { createNativeStackNavigator } from '@react-navigation/native-stack';
import { Pressable } from 'react-native';

import ModalScreen from '../screens/ModalScreen';
import NotFoundScreen from '../screens/NotFoundScreen';
import TabOneScreen from '../screens/TabOneScreen';
import TabTwoScreen from '../screens/TabTwoScreen';
import { RootStackParamList, RootTabParamList, RootTabScreenProps } from '../types/navigation';

const Stack = createNativeStackNavigator<RootStackParamList>();

// This is the main navigator we nest everything under.
export default function RootNavigator() {
	return (
		<Stack.Navigator>
			<Stack.Screen name="Root" component={BottomTabNavigator} options={{ headerShown: false }} />
			<Stack.Screen name="NotFound" component={NotFoundScreen} options={{ title: 'Oops!' }} />
			<Stack.Group screenOptions={{ presentation: 'modal' }}>
				<Stack.Screen name="Modal" component={ModalScreen} />
			</Stack.Group>
		</Stack.Navigator>
	);
}

const BottomTab = createBottomTabNavigator<RootTabParamList>();

function BottomTabNavigator() {
	return (
		<BottomTab.Navigator initialRouteName="TabOne">
			<BottomTab.Screen
				name="TabOne"
				component={TabOneScreen}
				options={({ navigation }: RootTabScreenProps<'TabOne'>) => ({
					title: 'Ball App',
					tabBarIcon: ({ color }) => (
						<FontAwesome size={30} style={{ marginBottom: -3 }} name="code" color={color} />
					),
					headerRight: () => (
						<Pressable
							onPress={() => navigation.navigate('Modal')}
							style={({ pressed }) => ({
								opacity: pressed ? 0.5 : 1
							})}
						>
							<FontAwesome
								name="info-circle"
								size={25}
								color={'purple'}
								style={{ marginRight: 15 }}
							/>
						</Pressable>
					)
				})}
			/>
			<BottomTab.Screen
				name="TabTwo"
				component={TabTwoScreen}
				options={{
					title: 'Ball Two',
					tabBarIcon: ({ color }) => (
						<FontAwesome size={30} style={{ marginBottom: -3 }} name="code" color={color} />
					)
				}}
			/>
		</BottomTab.Navigator>
	);
}
