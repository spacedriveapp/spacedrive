import { createNativeStackNavigator, NativeStackScreenProps } from '@react-navigation/native-stack';
import React from 'react';
import Header from '~/components/header/Header';
import FiltersScreen from '~/screens/search/Filters';
import SearchScreen from '~/screens/search/Search';

const Stack = createNativeStackNavigator<SearchStackParamList>();

export default function SearchStack() {
	return (
		<Stack.Navigator initialRouteName="Search">
			<Stack.Screen
				name="Search"
				component={SearchScreen}
				options={{
					headerShown: false
				}}
			/>
			<Stack.Screen
				name="Filters"
				component={FiltersScreen}
				options={{
					header: () => {
						return <Header navBack title="Search filters" />;
					}
				}}
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
