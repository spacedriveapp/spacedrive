import { createNativeStackNavigator, NativeStackScreenProps } from '@react-navigation/native-stack';
import React from 'react';
import Header from '~/components/header/Header';
import SearchScreen from '~/screens/search';
import FiltersScreen from '~/screens/search/Filters';

const Stack = createNativeStackNavigator<SearchStackParamList>();

export default function SearchStack() {
	return (
		<Stack.Navigator initialRouteName="Home">
			<Stack.Screen
				name="Home"
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
						return <Header navBack showSearch={false} title="Search filters" />;
					}
				}}
			/>
		</Stack.Navigator>
	);
}

export type SearchStackParamList = {
	Home: undefined;
	Filters: undefined;
};

export type SearchStackScreenProps<Screen extends keyof SearchStackParamList> =
	NativeStackScreenProps<SearchStackParamList, Screen>;
