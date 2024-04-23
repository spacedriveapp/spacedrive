import { createNativeStackNavigator, NativeStackScreenProps } from '@react-navigation/native-stack';
import React from 'react';
import FiltersScreen from '~/screens/search/Filters';
import SearchScreen from '~/screens/search/Search';

const Stack = createNativeStackNavigator<SearchStackParamList>();

export default function SearchStack() {
	return (
		<Stack.Navigator
		screenOptions={{
			headerShown: false
		}}
		initialRouteName="Search">
			<Stack.Screen
				name="Search"
				component={SearchScreen}
			/>
			<Stack.Screen
				name="Filters"
				component={FiltersScreen}
			/>
		</Stack.Navigator>
	);
}

export type SearchStackParamList = {
	Search: undefined;
	Filters: undefined;
};

export type SearchStackScreenProps<Screen extends keyof SearchStackParamList> =
	NativeStackScreenProps<SearchStackParamList, Screen>;
