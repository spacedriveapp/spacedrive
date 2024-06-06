import { createNativeStackNavigator, NativeStackScreenProps } from '@react-navigation/native-stack';
import React from 'react';
import DynamicHeader from '~/components/header/DynamicHeader';
import Header from '~/components/header/Header';
import LocationScreen from '~/screens/browse/Location';
import FiltersScreen from '~/screens/search/Filters';
import SearchScreen from '~/screens/search/Search';

const Stack = createNativeStackNavigator();

export default function BackfillWaitingStack() {
	return (
		<Stack.Navigator initialRouteName="BackfillWaiting">
			<></>
		</Stack.Navigator>
	);
}
