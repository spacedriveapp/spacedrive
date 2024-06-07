import { createNativeStackNavigator, NativeStackScreenProps } from '@react-navigation/native-stack';
import React from 'react';
import BackfillWaiting from '~/screens/BackfillWaiting';

const Stack = createNativeStackNavigator<BackfillWaitingStackParamList>();

export default function BackfillWaitingStack() {
	return (
		<Stack.Navigator initialRouteName="BackfillWaiting">
			<Stack.Screen
				name="BackfillWaiting"
				component={BackfillWaiting}
				options={{
					headerShown: false
				}}
			/>
		</Stack.Navigator>
	);
}

export type BackfillWaitingStackParamList = {
	BackfillWaiting: undefined;
};

export type BackfillWaitingStackScreenProps<Screen extends keyof BackfillWaitingStackParamList> =
	NativeStackScreenProps<BackfillWaitingStackParamList, Screen>;
